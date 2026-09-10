#!/bin/sh
# Drift check for the captured sample outputs (files 2–4 and 6–7). Run from
# the repo root: sh samples/regenerate.sh
set -e
cargo run -q -- samples/1.full-article.md > samples/.tmp-toc
diff samples/.tmp-toc samples/2.full-toc.md
cargo run -q -- samples/1.full-article.md -f "acid|pollution" > samples/.tmp-filtered
diff samples/.tmp-filtered samples/3.filtered-toc.md
cargo run -q -- samples/1.full-article.md 445-472,518-534,589-630 > samples/.tmp-extracted
diff samples/.tmp-extracted samples/4.extracted-content.md
cargo run -q -- samples/5.data-images.md > samples/.tmp-toc5
diff samples/.tmp-toc5 samples/6.data-images-toc.md
cargo run -q -- samples/5.data-images.md 5-20,30-41,42-50 > samples/.tmp-extracted5
diff samples/.tmp-extracted5 samples/7.data-images-extracted.md
rm -f samples/.tmp-toc samples/.tmp-filtered samples/.tmp-extracted samples/.tmp-toc5 samples/.tmp-extracted5
echo "samples in sync"
