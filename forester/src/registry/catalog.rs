use crate::registry::RegistryUrl;
use bon::Builder;
use nutype::nutype;
use reqwest::blocking::Client;
use serde::Deserialize;
use snafu::{ResultExt, Snafu};

#[nutype(
    validate(not_empty),
    derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Serialize,
        Deserialize,
        AsRef
    )
)]
pub struct RepositoryName(String);

#[nutype(
    validate(not_empty),
    derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Serialize,
        Deserialize,
        AsRef
    )
)]
pub struct ImageTag(String);

/// Manifest information for an image
#[derive(Debug, Clone, Builder)]
struct ImageManifest {
    size_bytes: u64,
    digest: String,
}

/// Represents an image in the registry with its repository and tag
#[derive(Debug, Clone, PartialEq, Eq, Builder)]
#[non_exhaustive]
pub struct RegistryImage {
    pub repository: RepositoryName,
    pub tag: ImageTag,
    pub size_bytes: u64,
    pub digest: String,
}

/// List all images in the registry
///
/// Queries the registry catalog API to discover all repositories and their tags.
pub fn list_images(registry_url: &RegistryUrl) -> Result<Vec<RegistryImage>, CatalogError> {
    let client = Client::new();

    let repositories = fetch_catalog(&client, registry_url)?;

    let mut images = Vec::new();
    for repo in repositories {
        let tags = fetch_tags(&client, registry_url, &repo)?;
        for tag in tags {
            let manifest = fetch_manifest_info(&client, registry_url, &repo, &tag)?;
            images.push(
                RegistryImage::builder()
                    .repository(repo.clone())
                    .tag(tag)
                    .size_bytes(manifest.size_bytes)
                    .digest(manifest.digest)
                    .build(),
            );
        }
    }

    images.sort_by(|a, b| {
        a.repository
            .cmp(&b.repository)
            .then_with(|| a.tag.cmp(&b.tag))
    });

    Ok(images)
}

fn fetch_catalog(
    client: &Client,
    registry_url: &RegistryUrl,
) -> Result<Vec<RepositoryName>, CatalogError> {
    use catalog_error::*;

    let url = format!("{}/v2/_catalog", registry_url);

    let response = client.get(&url).send().context(ApiRequestSnafu)?;

    snafu::ensure!(
        response.status().is_success(),
        UnexpectedResponseSnafu {
            status: response.status().as_u16()
        }
    );

    let catalog: CatalogResponse = response.json().context(ResponseParseSnafu)?;

    catalog
        .repositories
        .into_iter()
        .map(|name| {
            RepositoryName::try_new(name).map_err(|e| CatalogError::InvalidRepositoryName {
                name: e.to_string(),
            })
        })
        .collect()
}

fn fetch_tags(
    client: &Client,
    registry_url: &RegistryUrl,
    repository: &RepositoryName,
) -> Result<Vec<ImageTag>, CatalogError> {
    use catalog_error::*;

    let url = format!("{}/v2/{}/tags/list", registry_url, repository.as_ref());

    let response = client.get(&url).send().context(ApiRequestSnafu)?;

    snafu::ensure!(
        response.status().is_success(),
        UnexpectedResponseSnafu {
            status: response.status().as_u16()
        }
    );

    let tags_list: TagsListResponse = response.json().context(ResponseParseSnafu)?;

    tags_list
        .tags
        .unwrap_or_default()
        .into_iter()
        .map(|tag| {
            ImageTag::try_new(tag).map_err(|e| CatalogError::InvalidTag { tag: e.to_string() })
        })
        .collect()
}

fn fetch_manifest_info(
    client: &Client,
    registry_url: &RegistryUrl,
    repository: &RepositoryName,
    tag: &ImageTag,
) -> Result<ImageManifest, CatalogError> {
    use catalog_error::*;

    let url = format!(
        "{}/v2/{}/manifests/{}",
        registry_url,
        repository.as_ref(),
        tag.as_ref()
    );

    let response = client
        .get(&url)
        .header(
            "Accept",
            "application/vnd.oci.image.index.v1+json,application/vnd.oci.image.manifest.v1+json,application/vnd.docker.distribution.manifest.v2+json",
        )
        .send()
        .context(ApiRequestSnafu)?;

    snafu::ensure!(
        response.status().is_success(),
        UnexpectedResponseSnafu {
            status: response.status().as_u16()
        }
    );

    let digest = response
        .headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let manifest: ManifestResponse = response.json().context(ResponseParseSnafu)?;

    let total_size = if !manifest.manifests.is_empty() {
        // OCI index - sum all manifests
        manifest.manifests.iter().map(|m| m.size).sum()
    } else {
        // Regular manifest - sum config + layers
        manifest.config.map(|c| c.size).unwrap_or(0)
            + manifest.layers.iter().map(|layer| layer.size).sum::<u64>()
    };

    Ok(ImageManifest::builder()
        .size_bytes(total_size)
        .digest(digest)
        .build())
}

#[derive(Deserialize)]
struct CatalogResponse {
    repositories: Vec<String>,
}

#[derive(Deserialize)]
struct TagsListResponse {
    #[allow(dead_code)]
    name: String,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ManifestResponse {
    #[serde(default)]
    config: Option<Descriptor>,
    #[serde(default)]
    layers: Vec<Descriptor>,
    #[serde(default)]
    manifests: Vec<Descriptor>,
}

#[derive(Deserialize)]
struct Descriptor {
    size: u64,
}

#[derive(Debug, Snafu)]
#[snafu(module)]
pub enum CatalogError {
    #[snafu(display("Failed to query registry API"))]
    ApiRequest { source: reqwest::Error },

    #[snafu(display("Failed to parse registry response"))]
    ResponseParse { source: reqwest::Error },

    #[snafu(display("Registry returned unexpected status: {status}"))]
    UnexpectedResponse { status: u16 },

    #[snafu(display("Invalid repository name: {name}"))]
    InvalidRepositoryName { name: String },

    #[snafu(display("Invalid tag: {tag}"))]
    InvalidTag { tag: String },
}
