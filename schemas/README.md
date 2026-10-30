# Schemas

This folder holds OpenAPI documents used for command generation during development.

`samples/oatty-example.json` is an original Oatty example covered by the repository's
[MIT license](../LICENSE). It demonstrates listing, creating, and retrieving projects,
including query parameters, a path parameter, a JSON request body, and reusable response
schemas. Import it from the TUI Library view or from the repository root:

```bash
cargo run -p oatty -- import schemas/samples/oatty-example.json --kind catalog
```

The API is illustrative: no server implementation is supplied. Importing the document
does not contact its local `http://127.0.0.1:4010` endpoint; executing its generated
commands requires a compatible local server. No credentials are required by this example.

Ways to turn schemas into CLI commands:
- Import an OpenAPI document from the TUI Library view; it generates and saves a registry catalog.
- Programmatically generate a catalog/manifest using the `oatty-registry-gen` crate.

## Third-party sample

`samples/adyen-account-service.json` is an unchanged copy of Adyen's Account API v6
[OpenAPI document](https://github.com/Adyen/adyen-openapi/blob/1d165a07700491fe3eff1e368ffe090a33251e19/json/AccountService-v6.json).
It is retained under Adyen's own MIT license, copyright (c) 2017 Adyen bv; the
repository's Oatty copyright notice does not replace that attribution. The exact
[upstream license](https://github.com/Adyen/adyen-openapi/blob/1d165a07700491fe3eff1e368ffe090a33251e19/LICENSE)
is preserved in [licenses/Adyen-MIT.txt](licenses/Adyen-MIT.txt).

The 1Password, Ably, and Render samples previously stored here were removed because
their redistribution license could not be verified. Obtain vendor specifications
directly from their publishers and review their applicable terms before redistributing them.
