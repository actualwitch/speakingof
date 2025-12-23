rm -rf ./target
cargo leptos build --release
PRERENDER_ONLY=true ./target/release/speakingof
mkdir target/site/bunnycdn_errors
cp target/site/index.html target/site/bunnycdn_errors/404.html