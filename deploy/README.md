# Deploying digiflazz-service behind nginx

Target: service reachable at `https://dfsdk.ttmi.my.id` by browser clients.

Two steps need root. You run those yourself; everything else is already done.

## 1. What is already in place

- DNS `dfsdk.ttmi.my.id` → `43.156.92.36` (this host).
- Service listens on `0.0.0.0:8098`, managed by the systemd user unit
  `digiflazz-service.service` (`Restart=always`, linger enabled).
- CORS support in the service, restricted to `CORS_ORIGINS`.

## 2. Allow the browser origin

CORS is off unless origins are listed. Add them and restart (no sudo):

```sh
cd /home/ubuntu/app/digiflazz-rust
# edit .env, then:
systemctl --user restart digiflazz-service
```

Add this line to `.env`:

```
CORS_ORIGINS=https://app.example.com,https://dfsdk.ttmi.my.id
```

Guidance:

- List every origin exactly, including scheme and port. No wildcards; an
  invalid entry is logged and skipped at startup.
- Include only the sites that actually call the API. Each added origin can read
  responses with the bearer token the browser holds.
- To disable CORS again, remove the line.

Verify:

```sh
curl -i -X OPTIONS https://dfsdk.ttmi.my.id/transaction \
  -H 'Origin: https://app.example.com' \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: authorization,content-type'
# expect: access-control-allow-origin: https://app.example.com
```

## 3. Install the nginx site (needs sudo)

The config is committed at `deploy/nginx/dfsdk.ttmi.my.id.conf`.

**It is HTTP-only on purpose.** `certbot --nginx` runs `nginx -t` *before* it
issues a certificate, so a `443` block pointing at a not-yet-existing
`fullchain.pem` makes both `nginx -t` and certbot fail. Do not add a TLS block
by hand before the certificate exists.

```sh
cd /home/ubuntu/app/digiflazz-rust

sudo cp deploy/nginx/dfsdk.ttmi.my.id.conf /etc/nginx/sites-available/
sudo ln -sf /etc/nginx/sites-available/dfsdk.ttmi.my.id.conf /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

If a broken (TLS-referencing) site is already enabled, remove it before
reloading, otherwise nginx keeps running on its old in-memory config and the
next reload or reboot will take **every** site on the host down:

```sh
sudo rm -f /etc/nginx/sites-enabled/dfsdk.ttmi.my.id.conf
sudo nginx -t && sudo systemctl reload nginx
```

At this point the site answers on plain HTTP (proxying to the service), which
is enough for the next step.

## 4. Issue the certificate (needs sudo)

```sh
sudo certbot --nginx -d dfsdk.ttmi.my.id
```

certbot edits the site file itself, adding the `443` block and redirect. Then
confirm:

```sh
sudo nginx -t && sudo systemctl reload nginx
curl -sS https://dfsdk.ttmi.my.id/health
```

## 5. Firewall

If cloud security group / ufw only allows 80 and 443, you do **not** need to
open 8098: nginx reaches it over loopback. Do not expose 8098 publicly; the
service has no TLS of its own.

## 6. Verify from outside

```sh
curl -sS https://dfsdk.ttmi.my.id/health
# {"status":"ok","version":"0.1.12"}

curl -sS -H "Authorization: Bearer $TOKEN" https://dfsdk.ttmi.my.id/balance
# {"deposit":0.0}
```

---

# Before you wire a browser to this

Putting a bearer token in browser JavaScript means any visitor can read it from
the bundle or devtools and then spend your balance. The token is a secret with
the same power as your Digiflazz API key for these endpoints.

Recommended shape:

```
browser -> your web backend (session auth) -> digiflazz-service (bearer token)
```

The token stays server-side; the browser never sees it. In that layout the
browser talks to your own backend on the same origin, and `dfsdk.ttmi.my.id`
is called server-to-server, so CORS is not needed at all.

If you must call it directly from the browser, then:

- Use a **separate, low-privilege token** dedicated to that site, not the main
  service token.
- Never expose `POST /transaction` or `POST /deposit` to browsers at all.
  Expose only read endpoints, and only if the data is not sensitive.
- Accept that the token is public. Anyone can call `GET /balance` and
  `GET /price-list` with it, and consume your Digiflazz rate limit.
- Consider putting an authenticating gateway in front (an nginx `auth_request`
  or your own backend) instead of shipping a static token.
