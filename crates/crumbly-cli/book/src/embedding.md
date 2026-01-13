# Embedding

Embedding is where the "semantic" in semantic search comes from.
It's the step that lets crumbly understand what your documentation *means*, not just what words it contains.

## Text to Numbers

At its core, embedding converts text into a list of numbers called a *vector*.
Think of it like giving each chunk of text a unique fingerprint that captures its meaning.

```
"How to configure logging"  →  [0.12, -0.34, 0.56, ...]
"Setting up log output"     →  [0.11, -0.33, 0.55, ...]
"Unrelated topic"           →  [0.89, 0.12, -0.67, ...]
```

Notice how the first two vectors are similar—their numbers are close.
That's because they're about the same concept, even though they use different words.

## Why This Enables Semantic Search

Traditional keyword search fails when you use different words than the documentation:

- You search: "how do I set up logs"
- Doc says: "configuring the logging subsystem"
- Keyword search: ❌ no match
- Semantic search: ✅ finds it

Because embeddings capture *meaning*, similar concepts end up near each other in vector space.
When you search, crumbly embeds your query and finds chunks with similar vectors—regardless of exact wording.

```
┌─────────────────────────────────────────┐
│           Vector Space                  │
│                                         │
│    "logging config" •                   │
│                      • "log setup"      │
│    "configure logs" •                   │
│                                         │
│                                         │
│              • "database schema"        │
│                                         │
└─────────────────────────────────────────┘

Similar meanings cluster together.
```

## The Model

Crumbly uses `all-MiniLM-L6-v2`, a compact embedding model that runs locally.
It produces 384-dimensional vectors—enough to capture nuanced meaning while staying fast.

You don't need an API key or internet connection.
The model runs entirely on your machine.

## Automatic Operation

You don't need to configure embedding at all.
When you run `crumbly build`, it automatically:

1. Takes each chunk from the chunking stage
2. Converts it to a 384-number vector
3. Stores the vector for later searching

The embedding step is invisible—it just works.
Your only interaction is running `build` and `search`.

## Content-Addressed Storage

Crumbly is smart about duplicate content.
If two files contain identical text, they share the same embedding.
This saves storage and speeds up indexing when content hasn't changed.
