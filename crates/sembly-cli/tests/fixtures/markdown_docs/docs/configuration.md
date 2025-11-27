# Configuration Guide

This guide explains how to configure application settings.

## Database Settings

Configure database connections using the config file:

```toml
[database]
host = "localhost"
port = 5432
```

## Cache Settings

For distributed deployments, configure the cache layer:

```toml
[cache]
backend = "redis"
url = "redis://localhost:6379"
```

## Worker Settings

Background workers can be configured:

```toml
[workers]
count = 4
queue-size = 1000
```
