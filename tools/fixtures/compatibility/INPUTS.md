# Pinned compatibility fixture inputs

These five unmodified browser assets are test inputs in a caller-supplied
external cache. They are not ChezCat runtime dependencies or release assets.
The scorecard checks exact byte lengths and SHA-256 hashes before serving local
fixtures. It does not download, install or execute package managers or build
scripts. Full preserved MIT notices are beside this manifest under `licenses/`.

The cache layout is:

```text
<CACHE>/
  jquery-1.12.4/
    jquery-1.12.4.js
    LICENSE.txt
  bootstrap-3.4.1/
    bootstrap-3.4.1.js
    bootstrap-3.4.1.css
    LICENSE
  foundation-sites-6.9.0/
    foundation.min.css
    LICENSE
  vue-2.7.16/
    vue.runtime.min.js
    LICENSE
```

`inputs.json` records official distribution URLs or official npm archive URLs
with exact archive member names and package archive hashes/integrity. Only the
specified browser files and license texts are needed; no package installation
is required. The Vue input is the runtime build, without its template compiler.
Foundation coverage uses the compiled CSS input. Bootstrap's JavaScript fixture
uses the pinned jQuery input. Those choices bound the fixture scope and do not
claim support for the libraries as a whole.

Preserve upstream file bytes, attribution headers and license notices. Cache
paths in the manifest are relative to `<CACHE>`; select its location explicitly
when running the offline tool. Reports should name the exact selected assets
and observed workflow stages, separately from unsupported APIs and harness
limitations.
