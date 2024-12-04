# Changelog

## [1.4.1](https://github.com/semiotic-ai/elric-rs/compare/v1.4.0...v1.4.1) (2024-07-03)


### Bug Fixes

* update to use latest clickhouse version ([2fdc5fb](https://github.com/semiotic-ai/elric-rs/commit/2fdc5fb13d7950a828555ca268e7be6a16ecd4d5))

## [1.4.0](https://github.com/semiotic-ai/elric-rs/compare/v1.3.1...v1.4.0) (2023-09-18)


### Features

* add better logging ([9b29ea1](https://github.com/semiotic-ai/elric-rs/commit/9b29ea1052bc33459744b04a6ef8cd164f680c1f))
* use tracing as logging ([cec9b03](https://github.com/semiotic-ai/elric-rs/commit/cec9b03d7d71707cb856425d7573c9f95da2f108))

## [1.3.1](https://github.com/semiotic-ai/elric-rs/compare/v1.3.0...v1.3.1) (2023-09-04)


### Bug Fixes

* missing quote ([9cd8152](https://github.com/semiotic-ai/elric-rs/commit/9cd815240900919a1c11f668cf15299b00bb7b79))

## [1.3.0](https://github.com/semiotic-ai/elric-rs/compare/v1.2.1...v1.3.0) (2023-09-04)


### Features

* add better error handling ([d529066](https://github.com/semiotic-ai/elric-rs/commit/d529066d533fa6b5710d0df8d42d421bdd53e403))

## [1.2.1](https://github.com/semiotic-ai/elric-rs/compare/v1.2.0...v1.2.1) (2023-08-16)


### Bug Fixes

* **cursor:** update cursor only in processed block ([4b97fcb](https://github.com/semiotic-ai/elric-rs/commit/4b97fcb14b6086640765ca82776e8ab5e96a6342))
* **cursor:** use inserter with timeout ([5467b6c](https://github.com/semiotic-ai/elric-rs/commit/5467b6c87e0f11bdc94db888804bcced4ca827ee))

## [1.2.0](https://github.com/semiotic-ai/elric-rs/compare/v1.1.5...v1.2.0) (2023-08-16)


### Features

* add undo buffer and tests ([a6ef8ff](https://github.com/semiotic-ai/elric-rs/commit/a6ef8ff948a235161d1bd63210a6b66dadea0fbf))
* use final_block false ([59abe85](https://github.com/semiotic-ai/elric-rs/commit/59abe85622b1e0502a2c5c6b46a04e8c17581809))


### Bug Fixes

* **table_info:** add decimal to unimplemented ([a6f6ece](https://github.com/semiotic-ai/elric-rs/commit/a6f6ece3e75dbd56651dcc76e9ab4805e4be6fdf))

## [1.1.5](https://github.com/semiotic-ai/elric-rs/compare/v1.1.4...v1.1.5) (2023-08-14)


### Bug Fixes

* **schema:** use schema instead of using default ([dbc7f7a](https://github.com/semiotic-ai/elric-rs/commit/dbc7f7a029c17d566b9db74be01bc48533458cdc))

## [1.1.4](https://github.com/semiotic-ai/elric-rs/compare/v1.1.3...v1.1.4) (2023-08-14)


### Bug Fixes

* **docker:** use slim instead of alpine for now ([b843f85](https://github.com/semiotic-ai/elric-rs/commit/b843f853ef77b8c807e6c16398ace8a7dbdbb4b3))

## [1.1.3](https://github.com/semiotic-ai/elric-rs/compare/v1.1.2...v1.1.3) (2023-08-14)


### Bug Fixes

* **ssl:** remove all dependencies ([8ba7d3f](https://github.com/semiotic-ai/elric-rs/commit/8ba7d3fe61ef47903a30dc756e3942785291a710))

## [1.1.2](https://github.com/semiotic-ai/elric-rs/compare/v1.1.1...v1.1.2) (2023-08-14)


### Bug Fixes

* **docker:** update readme ([d22135a](https://github.com/semiotic-ai/elric-rs/commit/d22135aed3da748daca1dee842069bdd78ee10c0))
* **ssl:** remove openssl dependency ([eefa875](https://github.com/semiotic-ai/elric-rs/commit/eefa875d619aed8a95b95065e33fbb639c15db6c))

## [1.1.1](https://github.com/semiotic-ai/elric-rs/compare/v1.1.0...v1.1.1) (2023-08-11)


### Bug Fixes

* **docker:** use openssl vendored ([855bd89](https://github.com/semiotic-ai/elric-rs/commit/855bd89009b337cc97acb571c3f22c44336b9d4a))

## [1.1.0](https://github.com/semiotic-ai/elric-rs/compare/v1.0.4...v1.1.0) (2023-08-11)


### Features

* **setup:** add setup command ([22cfce2](https://github.com/semiotic-ai/elric-rs/commit/22cfce2a30fd37cb6adbc187354b2fc4f554cf2a))

## [1.0.4](https://github.com/semiotic-ai/elric-rs/compare/v1.0.3...v1.0.4) (2023-08-11)


### Bug Fixes

* update application name in docker ([d98d353](https://github.com/semiotic-ai/elric-rs/commit/d98d35339ba9ac18ec3600e2aaaab7726b1cea4f))

## [1.0.3](https://github.com/semiotic-ai/elric-rs/compare/v1.0.2...v1.0.3) (2023-08-11)


### Bug Fixes

* update token lifetime ([0443b9a](https://github.com/semiotic-ai/elric-rs/commit/0443b9a4854216680146d3f5d86d790b34fa3f98))

## [1.0.2](https://github.com/semiotic-ai/elric-rs/compare/v1.0.1...v1.0.2) (2023-08-11)


### Bug Fixes

* delete commented code ([3671450](https://github.com/semiotic-ai/elric-rs/commit/367145031e93c11848d1b4ac0994924f4bfce91e))

## [1.0.1](https://github.com/semiotic-ai/elric-rs/compare/v1.0.0...v1.0.1) (2023-08-11)


### Bug Fixes

* remove u256 mod ([fbde4d0](https://github.com/semiotic-ai/elric-rs/commit/fbde4d04af854ee8693ff695d908fc209fe0183c))
