default:
    @just --list

dist: clean build-release
    @mkdir -p dist
    @cp target/release/wibble dist/
    @cp -r static dist/
    @cp -r templates dist/
    @echo "Distribution created in dist/"

clean:
    @cargo clean
    @rm -rf dist

build-release:
    @cargo build --release

tar: dist
    @tar -czf wibble-dist.tar.gz -C dist .
    @echo "Distribution tarball created: wibble-dist.tar.gz"
