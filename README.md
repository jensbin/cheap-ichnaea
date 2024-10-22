# What is good for?

As a geolocation service to feed geloclue2. I only use geoclue2 to update the
timezone information on my device.
With the retirement of the Mozilla Location Service
(https://github.com/mozilla/ichnaea/issues/2065) the only option was to move
to Google Location Service.

I don't need accurate location data for timezone, the
[ip-api](https://ip-api.com service) service suits well enough.
So I build this local service.

# How to use it?
Start the service and adjust the `url` in `/etc/geoclue/geoclue.conf`

```
url=http://localhost:8080/v1/geolocate
```

```
Usage: cheap-ichnaea [OPTIONS]

Options:
  -p, --port <PORT>            The port number to listen on [default: 8080]
  -t, --ttl-cache <TTL_CACHE>  Cache TTL in seconds [default: 1800]
  -h, --help                   Print help
  -V, --version                Print version
```

# Features

- Caching of responses (`--ttl-cache 3200`)
- Retrying to connect, when connection fails