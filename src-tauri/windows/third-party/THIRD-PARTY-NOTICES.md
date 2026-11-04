# Drittanbieterhinweise

Version 1.0.0-alpha.4, Windows x64. Vollständige Lizenztexte stehen unter cargo/ und npm/. Das maschinenlesbare Inventar inventory.json bindet jede Textdatei an ihre SHA256. Cargo umfasst konservativ auch Bau- und Testabhängigkeiten; diese werden nicht pauschal als verteilte Binärkomponenten bezeichnet. npm-Entwicklungswerkzeuge werden nicht mit der App verteilt.

Die unveränderten, prüfsummengeprüften Quellen aller aufgelösten MPL-2.0-Crates liegen als .crate-Archive unter sources/. Diese sind gzip-komprimierte tar-Archive. Es wurden keine Fremdquellen geändert. Das Archiv enthält jeweils die originale Cargo.toml, Herkunftsmetadaten und Quelltexte. Für selectors ergänzt der unveränderte Standardtext der MPL 2.0 aus cssparser die Lizenzdeklaration und Quelltextköpfe.

Die Hinweise der verwendeten Rust-Standardbibliothek stehen unter rust-standard-library/COPYRIGHT-library.html. Beiliegende Standardlizenztexte sind eine konservative Sammlung; die dortige Komponentenzuordnung bestimmt die jeweilige Lizenz. Die Compilerkennung und Dateihashes stehen im Inventar.

Der statisch eingebundene Microsoft-WebView2-Loader aus SDK 1.0.3800.47 ist unter native/microsoft-webview2-loader-1.0.3800.47/ gesondert erfasst. LICENSE.txt und NOTICE.txt sind unverändert aus dem offiziellen NuGet-Paket übernommen. Der x64-Loader stimmt bytegenau mit der von webview2-com-sys 0.39.1 eingebundenen Bibliothek überein; provenance.json dokumentiert Paket- und Bibliothekshash. Die vollständigen SDK-Hinweise sind konservativ beigefügt und behaupten nicht, dass jede darin erwähnte Komponente eingebunden ist.

WebView2 Evergreen wird als externe Windows-Laufzeit vorausgesetzt beziehungsweise vom Installer über Microsoft bezogen; dessen Laufzeitdateien sind nicht Bestandteil dieses Pakets. Eigener Projektcode erhält durch dieses Inventar keine zusätzliche Lizenz. Die Tauri-NSIS-Lizenz steht im Quellcode unter [LICENSE-MIT-TAURI.txt](../LICENSE-MIT-TAURI.txt) und im Installationsordner unter licenses/TAURI-NSIS-MIT.txt.

| Ökosystem | Paket | Version | SPDX-Angabe des Pakets | Umfang |
|---|---|---|---|---|
| Cargo | adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | aho-corasick | 1.1.5 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | alloc-no-stdlib | 3.0.0 | BSD-3-Clause | release-normal-dependency-closure |
| Cargo | alloc-stdlib | 0.3.0 | BSD-3-Clause | release-normal-dependency-closure |
| Cargo | anyhow | 1.0.104 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | autocfg | 1.5.1 | Apache-2.0 OR MIT | release-build-dependency-closure |
| Cargo | base64 | 0.23.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | bit-set | 0.8.0 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | bit-vec | 0.8.0 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | bitflags | 1.3.2 | MIT/Apache-2.0 | release-normal-dependency-closure |
| Cargo | bitflags | 2.13.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | block-buffer | 0.10.4 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | brotli | 9.1.0 | BSD-3-Clause AND MIT | release-normal-dependency-closure |
| Cargo | brotli-decompressor | 6.2.0 | BSD-3-Clause/MIT | release-normal-dependency-closure |
| Cargo | bs58 | 0.5.1 | MIT/Apache-2.0 | release-normal-dependency-closure |
| Cargo | bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | byteorder | 1.5.0 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | byteorder-lite | 0.1.0 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | bytes | 1.12.1 | MIT | release-normal-dependency-closure |
| Cargo | camino | 1.2.6 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | cargo-platform | 0.1.9 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | cargo_metadata | 0.19.2 | MIT | release-normal-dependency-closure |
| Cargo | cargo_toml | 1.0.1 | Apache-2.0 OR MIT | release-build-dependency-closure |
| Cargo | cc | 1.10.0 | MIT OR Apache-2.0 | release-build-dependency-closure |
| Cargo | cfb | 0.14.0 | MIT | release-normal-dependency-closure |
| Cargo | cfg-if | 1.0.5 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | chrono | 0.4.45 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | cookie | 0.18.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | cpufeatures | 0.2.17 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | crc32fast | 1.5.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | crypto-common | 0.1.7 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | cssparser | 0.37.0 | MPL-2.0 | release-normal-dependency-closure |
| Cargo | cssparser-macros | 0.7.1 | MPL-2.0 | release-normal-dependency-closure |
| Cargo | ctor | 1.0.13 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | darling | 0.24.1 | MIT | release-normal-dependency-closure |
| Cargo | darling_core | 0.24.1 | MIT | release-normal-dependency-closure |
| Cargo | darling_macro | 0.24.1 | MIT | release-normal-dependency-closure |
| Cargo | defmt | 1.2.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | defmt-macros | 1.2.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | defmt-parser | 1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | deranged | 0.5.9 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | derive_more | 2.1.1 | MIT | release-normal-dependency-closure |
| Cargo | derive_more-impl | 2.1.1 | MIT | release-normal-dependency-closure |
| Cargo | digest | 0.10.7 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | dirs | 7.0.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | dirs-sys | 0.5.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | displaydoc | 0.2.7 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | dom_query | 0.28.0 | MIT | release-normal-dependency-closure |
| Cargo | dpi | 0.1.2 | Apache-2.0 AND MIT | release-normal-dependency-closure |
| Cargo | dtoa | 1.0.11 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | dtoa-short | 0.3.5 | MPL-2.0 | release-normal-dependency-closure |
| Cargo | dunce | 1.0.5 | CC0-1.0 OR MIT-0 OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | dyn-clone | 1.0.20 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | embed-resource | 3.0.12 | MIT | release-build-dependency-closure |
| Cargo | equivalent | 1.0.3 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | erased-serde | 0.4.11 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | fastrand | 2.5.0 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | fdeflate | 0.3.7 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | find-msvc-tools | 0.1.15 | MIT OR Apache-2.0 | release-build-dependency-closure |
| Cargo | flate2 | 1.1.10 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | fnv | 1.0.7 | Apache-2.0 / MIT | release-normal-dependency-closure |
| Cargo | foldhash | 0.2.0 | Zlib | release-normal-dependency-closure |
| Cargo | form_urlencoded | 1.2.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | generic-array | 0.14.7 | MIT | release-normal-dependency-closure |
| Cargo | getrandom | 0.3.4 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | getrandom | 0.4.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | glob | 0.3.4 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | hashbrown | 0.12.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | hashbrown | 0.17.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | heck | 0.5.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | hex | 0.4.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | html5ever | 0.39.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | http | 1.5.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | ico | 0.6.0 | MIT | release-normal-dependency-closure |
| Cargo | icu_collections | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | icu_locale_core | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | icu_normalizer | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | icu_normalizer_data | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | icu_properties | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | icu_properties_data | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | icu_provider | 2.4.0 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | ident_case | 1.0.1 | MIT/Apache-2.0 | release-normal-dependency-closure |
| Cargo | idna | 1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | idna_adapter | 1.2.2 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | image | 0.25.10 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | indexmap | 1.9.3 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | indexmap | 2.14.2 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | infer | 0.22.0 | MIT | release-normal-dependency-closure |
| Cargo | itoa | 1.0.18 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | jiff | 0.2.38 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | jiff-core | 0.1.1 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | jiff-tzdb | 0.1.9 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | jiff-tzdb-platform | 0.1.3 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | json-patch | 4.2.0 | MIT/Apache-2.0 | release-normal-dependency-closure |
| Cargo | jsonptr | 0.7.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | keyboard-types | 0.8.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | libc | 0.2.191 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | litemap | 0.8.3 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | lock_api | 0.4.14 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | log | 0.4.34 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | markup5ever | 0.39.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | memchr | 2.8.3 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | mime | 0.3.17 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | mio | 1.2.4 | MIT | release-normal-dependency-closure |
| Cargo | moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | muda | 0.20.0 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | new_debug_unreachable | 1.0.6 | MIT | release-normal-dependency-closure |
| Cargo | num-conv | 0.2.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | num-traits | 0.2.19 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | once_cell | 1.21.4 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | option-ext | 0.2.0 | MPL-2.0 | release-normal-dependency-closure |
| Cargo | parking_lot | 0.12.5 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | percent-encoding | 2.3.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | phf | 0.13.1 | MIT | release-normal-dependency-closure |
| Cargo | phf_codegen | 0.13.1 | MIT | release-build-dependency-closure |
| Cargo | phf_generator | 0.13.1 | MIT | release-normal-dependency-closure |
| Cargo | phf_macros | 0.13.1 | MIT | release-normal-dependency-closure |
| Cargo | phf_shared | 0.13.1 | MIT | release-normal-dependency-closure |
| Cargo | pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | plist | 1.10.3 | MIT | release-normal-dependency-closure |
| Cargo | png | 0.18.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | potential_utf | 0.1.6 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | powerfmt | 0.2.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | precomputed-hash | 0.1.1 | MIT | release-normal-dependency-closure |
| Cargo | proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | quick-xml | 0.42.0 | MIT | release-normal-dependency-closure |
| Cargo | quote | 1.0.47 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | release-normal-dependency-closure |
| Cargo | ref-cast | 1.0.27 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | ref-cast-impl | 1.0.27 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | regex | 1.13.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | regex-automata | 0.4.18 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | regex-syntax | 0.8.11 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | rustc-hash | 2.1.3 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | rustc_version | 0.4.1 | MIT OR Apache-2.0 | release-build-dependency-closure |
| Cargo | same-file | 1.0.6 | Unlicense/MIT | release-normal-dependency-closure |
| Cargo | schemars | 0.8.22 | MIT | release-normal-dependency-closure |
| Cargo | schemars | 0.9.0 | MIT | release-normal-dependency-closure |
| Cargo | schemars | 1.2.3 | MIT | release-normal-dependency-closure |
| Cargo | schemars_derive | 0.8.22 | MIT | release-normal-dependency-closure |
| Cargo | scopeguard | 1.2.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | selectors | 0.38.0 | MPL-2.0 | release-normal-dependency-closure |
| Cargo | semver | 1.0.28 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde | 1.0.229 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde-untagged | 0.1.9 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_core | 1.0.229 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_derive | 1.0.229 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_derive_internals | 0.29.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_json | 1.0.154 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_repr | 0.1.21 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_spanned | 1.1.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_with | 3.24.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serde_with_macros | 3.24.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serialize-to-javascript | 0.1.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | serialize-to-javascript-impl | 0.1.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | servo_arc | 0.4.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | sha2 | 0.10.9 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | shlex | 2.0.1 | MIT OR Apache-2.0 | release-build-dependency-closure |
| Cargo | simd-adler32 | 0.3.10 | MIT | release-normal-dependency-closure |
| Cargo | siphasher | 1.0.4 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | smallvec | 1.16.4 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | socket2 | 0.6.5 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | softbuffer | 0.4.8 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | static_vcruntime | 3.0.0 | MIT OR Apache-2.0 OR Zlib | release-build-dependency-closure |
| Cargo | string_cache | 0.9.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | string_cache_codegen | 0.6.1 | MIT OR Apache-2.0 | release-build-dependency-closure |
| Cargo | strsim | 0.11.1 | MIT | release-normal-dependency-closure |
| Cargo | syn | 2.0.119 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | syn | 3.0.9 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | synstructure | 0.14.0 | MIT | release-normal-dependency-closure |
| Cargo | tao | 0.37.2 | Apache-2.0 | release-normal-dependency-closure |
| Cargo | tauri | 2.12.5 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tauri-build | 2.7.4 | Apache-2.0 OR MIT | release-build-dependency-closure |
| Cargo | tauri-codegen | 2.7.4 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tauri-macros | 2.7.4 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tauri-runtime | 2.12.2 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tauri-runtime-wry | 2.12.3 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tauri-utils | 2.10.2 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tauri-winres | 0.3.7 | MIT | release-build-dependency-closure |
| Cargo | tendril | 0.5.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | thiserror | 2.0.21 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | time | 0.3.55 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | time-core | 0.1.9 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | time-macros | 0.2.32 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | tinystr | 0.8.5 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | tokio | 1.54.1 | MIT | release-normal-dependency-closure |
| Cargo | toml | 1.1.8+spec-1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | toml_writer | 1.1.3+spec-1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | tracing | 0.1.44 | MIT | release-normal-dependency-closure |
| Cargo | tracing-core | 0.1.36 | MIT | release-normal-dependency-closure |
| Cargo | tray-icon | 0.25.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | typeid | 1.0.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | typenum | 1.20.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | unicode-ident | 1.0.27 | (MIT OR Apache-2.0) AND Unicode-3.0 | release-normal-dependency-closure |
| Cargo | unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | url | 2.5.8 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | urlpattern | 0.6.0 | MIT | release-normal-dependency-closure |
| Cargo | utf8_iter | 1.0.4 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | uuid | 1.29.0 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | version_check | 0.9.5 | MIT/Apache-2.0 | release-build-dependency-closure |
| Cargo | vswhom | 0.1.0 | MIT | release-build-dependency-closure |
| Cargo | vswhom-sys | 0.1.3 | MIT | release-build-dependency-closure |
| Cargo | walkdir | 2.5.0 | Unlicense/MIT | release-normal-dependency-closure |
| Cargo | web-time | 1.1.0 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | web_atoms | 0.2.6 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | webview2-com | 0.39.1 | MIT | release-normal-dependency-closure |
| Cargo | webview2-com-macros | 0.8.1 | MIT | release-normal-dependency-closure |
| Cargo | webview2-com-sys | 0.39.1 | MIT | release-normal-dependency-closure |
| Cargo | winapi-util | 0.1.11 | Unlicense OR MIT | release-normal-dependency-closure |
| Cargo | window-vibrancy | 0.8.1 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | windows | 0.62.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-collections | 0.3.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-core | 0.62.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-future | 0.3.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-implement | 0.60.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-interface | 0.59.3 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-link | 0.2.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-numerics | 0.3.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-result | 0.4.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-strings | 0.5.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-sys | 0.61.2 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-threading | 0.2.1 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | windows-version | 0.1.7 | MIT OR Apache-2.0 | release-normal-dependency-closure |
| Cargo | winnow | 1.0.4 | MIT | release-normal-dependency-closure |
| Cargo | winreg | 0.56.0 | MIT | release-build-dependency-closure |
| Cargo | writeable | 0.6.4 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | wry | 0.57.1 | Apache-2.0 OR MIT | release-normal-dependency-closure |
| Cargo | yoke | 0.8.4 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | yoke-derive | 0.8.4 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | zerofrom | 0.1.8 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | zerofrom-derive | 0.1.8 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | zerotrie | 0.2.5 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | zerovec | 0.11.9 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | zerovec-derive | 0.11.7 | Unicode-3.0 | release-normal-dependency-closure |
| Cargo | zlib-rs | 0.6.8 | Zlib | release-normal-dependency-closure |
| Cargo | zmij | 1.0.23 | MIT | release-normal-dependency-closure |
| npm | @adobe/css-tools | 4.5.0 | MIT | development-only-not-shipped |
| npm | @asamuzakjp/css-color | 3.2.0 | MIT | development-only-not-shipped |
| npm | @babel/code-frame | 7.29.7 | MIT | development-only-not-shipped |
| npm | @babel/helper-validator-identifier | 7.29.7 | MIT | development-only-not-shipped |
| npm | @babel/runtime | 7.29.10 | MIT | development-only-not-shipped |
| npm | @csstools/color-helpers | 5.1.0 | MIT-0 | development-only-not-shipped |
| npm | @csstools/css-calc | 2.1.4 | MIT | development-only-not-shipped |
| npm | @csstools/css-color-parser | 3.1.0 | MIT | development-only-not-shipped |
| npm | @csstools/css-parser-algorithms | 3.0.5 | MIT | development-only-not-shipped |
| npm | @csstools/css-tokenizer | 3.0.4 | MIT | development-only-not-shipped |
| npm | @eslint-community/eslint-utils | 4.10.1 | MIT | development-only-not-shipped |
| npm | eslint-visitor-keys | 3.4.3 | Apache-2.0 | development-only-not-shipped |
| npm | @eslint-community/regexpp | 4.12.2 | MIT | development-only-not-shipped |
| npm | @eslint/config-array | 0.21.2 | Apache-2.0 | development-only-not-shipped |
| npm | @eslint/config-helpers | 0.4.2 | Apache-2.0 | development-only-not-shipped |
| npm | @eslint/core | 0.17.0 | Apache-2.0 | development-only-not-shipped |
| npm | @eslint/eslintrc | 3.3.7 | MIT | development-only-not-shipped |
| npm | globals | 14.0.0 | MIT | development-only-not-shipped |
| npm | @eslint/js | 9.39.5 | MIT | development-only-not-shipped |
| npm | @eslint/object-schema | 2.1.7 | Apache-2.0 | development-only-not-shipped |
| npm | @eslint/plugin-kit | 0.4.1 | Apache-2.0 | development-only-not-shipped |
| npm | @humanfs/core | 0.19.2 | Apache-2.0 | development-only-not-shipped |
| npm | @humanfs/node | 0.16.8 | Apache-2.0 | development-only-not-shipped |
| npm | @humanfs/types | 0.15.0 | Apache-2.0 | development-only-not-shipped |
| npm | @humanwhocodes/module-importer | 1.0.1 | Apache-2.0 | development-only-not-shipped |
| npm | @humanwhocodes/retry | 0.4.3 | Apache-2.0 | development-only-not-shipped |
| npm | @isaacs/nonrecursive-glob-match | 1.0.3 | BlueOak-1.0.0 | development-only-not-shipped |
| npm | @jridgewell/resolve-uri | 3.1.2 | MIT | development-only-not-shipped |
| npm | @jridgewell/sourcemap-codec | 1.6.0 | MIT | development-only-not-shipped |
| npm | @jridgewell/trace-mapping | 0.3.31 | MIT | development-only-not-shipped |
| npm | @oxc-project/types | 0.155.0 | MIT | development-only-not-shipped |
| npm | @rolldown/binding-android-arm-eabi | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-android-arm64 | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-darwin-arm64 | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-darwin-x64 | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-freebsd-x64 | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-arm-gnueabihf | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-arm64-gnu | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-arm64-musl | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-ppc64-gnu | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-s390x-gnu | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-x64-gnu | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-linux-x64-musl | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-openharmony-arm64 | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-win32-arm64-msvc | 1.2.15 | MIT | development-only-not-shipped (not installed) |
| npm | @rolldown/binding-win32-x64-msvc | 1.2.15 | MIT | development-only-not-shipped |
| npm | @rolldown/pluginutils | 1.0.1 | MIT | development-only-not-shipped |
| npm | @tauri-apps/api | 2.12.4 | Apache-2.0 OR MIT | frontend-production |
| npm | @tauri-apps/cli | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped |
| npm | @tauri-apps/cli-darwin-arm64 | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-darwin-x64 | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-linux-arm-gnueabihf | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-linux-arm64-gnu | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-linux-arm64-musl | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-linux-riscv64-gnu | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-linux-x64-gnu | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-linux-x64-musl | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-win32-arm64-msvc | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-win32-ia32-msvc | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped (not installed) |
| npm | @tauri-apps/cli-win32-x64-msvc | 2.12.4 | Apache-2.0 OR MIT | development-only-not-shipped |
| npm | @testing-library/dom | 10.4.2 | MIT | development-only-not-shipped |
| npm | @testing-library/jest-dom | 6.9.1 | MIT | development-only-not-shipped |
| npm | dom-accessibility-api | 0.6.3 | MIT | development-only-not-shipped |
| npm | @testing-library/react | 16.3.3 | MIT | development-only-not-shipped |
| npm | @types/aria-query | 5.0.4 | MIT | development-only-not-shipped |
| npm | @types/chai | 5.2.3 | MIT | development-only-not-shipped |
| npm | @types/deep-eql | 4.0.2 | MIT | development-only-not-shipped |
| npm | @types/estree | 1.0.9 | MIT | development-only-not-shipped |
| npm | @types/json-schema | 7.0.15 | MIT | development-only-not-shipped |
| npm | @types/node | 22.20.5 | MIT | development-only-not-shipped |
| npm | @types/react | 19.3.0 | MIT | development-only-not-shipped |
| npm | @types/react-dom | 19.3.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/eslint-plugin | 8.72.0 | MIT | development-only-not-shipped |
| npm | ignore | 7.0.14 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/parser | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/project-service | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/scope-manager | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/tsconfig-utils | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/type-utils | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/types | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/typescript-estree | 8.72.0 | MIT | development-only-not-shipped |
| npm | balanced-match | 4.0.4 | MIT | development-only-not-shipped |
| npm | brace-expansion | 5.0.12 | MIT | development-only-not-shipped |
| npm | minimatch | 11.0.4 | BlueOak-1.0.0 | development-only-not-shipped |
| npm | @typescript-eslint/utils | 8.72.0 | MIT | development-only-not-shipped |
| npm | @typescript-eslint/visitor-keys | 8.72.0 | MIT | development-only-not-shipped |
| npm | eslint-visitor-keys | 5.0.1 | Apache-2.0 | development-only-not-shipped |
| npm | @vitejs/plugin-react | 6.1.2 | MIT | development-only-not-shipped |
| npm | @vitest/mocker | 5.0.5 | MIT | development-only-not-shipped |
| npm | @vitest/spy | 5.0.5 | MIT | development-only-not-shipped |
| npm | acorn | 8.19.0 | MIT | development-only-not-shipped |
| npm | acorn-jsx | 5.3.2 | MIT | development-only-not-shipped |
| npm | agent-base | 7.1.4 | MIT | development-only-not-shipped |
| npm | ajv | 6.15.0 | MIT | development-only-not-shipped |
| npm | ansi-regex | 5.0.1 | MIT | development-only-not-shipped |
| npm | ansi-styles | 4.3.0 | MIT | development-only-not-shipped |
| npm | argparse | 2.0.1 | Python-2.0 | development-only-not-shipped |
| npm | aria-query | 5.3.0 | Apache-2.0 | development-only-not-shipped |
| npm | assertion-error | 2.0.1 | MIT | development-only-not-shipped |
| npm | balanced-match | 1.0.2 | MIT | development-only-not-shipped |
| npm | brace-expansion | 1.1.21 | MIT | development-only-not-shipped |
| npm | callsites | 3.1.0 | MIT | development-only-not-shipped |
| npm | chai | 6.3.0 | MIT | development-only-not-shipped |
| npm | chalk | 4.1.2 | MIT | development-only-not-shipped |
| npm | color-convert | 2.0.1 | MIT | development-only-not-shipped |
| npm | color-name | 1.1.4 | MIT | development-only-not-shipped |
| npm | concat-map | 0.0.1 | MIT | development-only-not-shipped |
| npm | cross-spawn | 7.0.6 | MIT | development-only-not-shipped |
| npm | css.escape | 1.5.1 | MIT | development-only-not-shipped |
| npm | cssstyle | 4.6.0 | MIT | development-only-not-shipped |
| npm | csstype | 3.2.3 | MIT | development-only-not-shipped |
| npm | data-urls | 5.0.0 | MIT | development-only-not-shipped |
| npm | debug | 4.4.3 | MIT | development-only-not-shipped |
| npm | decimal.js | 10.6.0 | MIT | development-only-not-shipped |
| npm | deep-is | 0.1.4 | MIT | development-only-not-shipped |
| npm | dequal | 2.0.3 | MIT | development-only-not-shipped |
| npm | detect-libc | 2.1.2 | Apache-2.0 | development-only-not-shipped |
| npm | dom-accessibility-api | 0.5.16 | MIT | development-only-not-shipped |
| npm | entities | 6.0.1 | BSD-2-Clause | development-only-not-shipped |
| npm | es-module-lexer | 2.3.2 | MIT | development-only-not-shipped |
| npm | escape-string-regexp | 4.0.0 | MIT | development-only-not-shipped |
| npm | eslint | 9.39.5 | MIT | development-only-not-shipped |
| npm | eslint-plugin-react-hooks | 5.2.0 | MIT | development-only-not-shipped |
| npm | eslint-scope | 8.4.0 | BSD-2-Clause | development-only-not-shipped |
| npm | eslint-visitor-keys | 4.2.1 | Apache-2.0 | development-only-not-shipped |
| npm | espree | 10.4.0 | BSD-2-Clause | development-only-not-shipped |
| npm | esquery | 1.7.0 | BSD-3-Clause | development-only-not-shipped |
| npm | esrecurse | 4.3.0 | BSD-2-Clause | development-only-not-shipped |
| npm | estraverse | 5.3.0 | BSD-2-Clause | development-only-not-shipped |
| npm | estree-walker | 3.0.3 | MIT | development-only-not-shipped |
| npm | esutils | 2.0.3 | BSD-2-Clause | development-only-not-shipped |
| npm | expect-type | 1.4.0 | Apache-2.0 | development-only-not-shipped |
| npm | fast-deep-equal | 3.1.3 | MIT | development-only-not-shipped |
| npm | fast-json-stable-stringify | 2.1.0 | MIT | development-only-not-shipped |
| npm | fast-levenshtein | 2.0.6 | MIT | development-only-not-shipped |
| npm | fdir | 6.5.0 | MIT | development-only-not-shipped |
| npm | file-entry-cache | 8.0.0 | MIT | development-only-not-shipped |
| npm | find-up | 5.0.0 | MIT | development-only-not-shipped |
| npm | flat-cache | 4.0.1 | MIT | development-only-not-shipped |
| npm | flatted | 3.4.4 | ISC | development-only-not-shipped |
| npm | fsevents | 2.3.3 | MIT | development-only-not-shipped (not installed) |
| npm | glob-parent | 6.0.2 | ISC | development-only-not-shipped |
| npm | globals | 16.5.0 | MIT | development-only-not-shipped |
| npm | has-flag | 4.0.0 | MIT | development-only-not-shipped |
| npm | html-encoding-sniffer | 4.0.0 | MIT | development-only-not-shipped |
| npm | http-proxy-agent | 7.0.2 | MIT | development-only-not-shipped |
| npm | https-proxy-agent | 7.0.6 | MIT | development-only-not-shipped |
| npm | iconv-lite | 0.6.3 | MIT | development-only-not-shipped |
| npm | ignore | 5.3.2 | MIT | development-only-not-shipped |
| npm | import-fresh | 3.3.1 | MIT | development-only-not-shipped |
| npm | imurmurhash | 0.1.4 | MIT | development-only-not-shipped |
| npm | indent-string | 4.0.0 | MIT | development-only-not-shipped |
| npm | is-extglob | 2.1.1 | MIT | development-only-not-shipped |
| npm | is-glob | 4.0.3 | MIT | development-only-not-shipped |
| npm | is-potential-custom-element-name | 1.0.1 | MIT | development-only-not-shipped |
| npm | isexe | 2.0.0 | ISC | development-only-not-shipped |
| npm | js-tokens | 4.0.0 | MIT | development-only-not-shipped |
| npm | js-yaml | 4.3.2 | MIT | development-only-not-shipped |
| npm | jsdom | 26.1.0 | MIT | development-only-not-shipped |
| npm | json-buffer | 3.0.1 | MIT | development-only-not-shipped |
| npm | json-schema-traverse | 0.4.1 | MIT | development-only-not-shipped |
| npm | json-stable-stringify-without-jsonify | 1.0.1 | MIT | development-only-not-shipped |
| npm | keyv | 4.5.4 | MIT | development-only-not-shipped |
| npm | levn | 0.4.1 | MIT | development-only-not-shipped |
| npm | lightningcss | 1.33.0 | MPL-2.0 | development-only-not-shipped |
| npm | lightningcss-android-arm64 | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-darwin-arm64 | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-darwin-x64 | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-freebsd-x64 | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-linux-arm-gnueabihf | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-linux-arm64-gnu | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-linux-arm64-musl | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-linux-x64-gnu | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-linux-x64-musl | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-win32-arm64-msvc | 1.33.0 | MPL-2.0 | development-only-not-shipped (not installed) |
| npm | lightningcss-win32-x64-msvc | 1.33.0 | MPL-2.0 | development-only-not-shipped |
| npm | locate-path | 6.0.0 | MIT | development-only-not-shipped |
| npm | lodash.merge | 4.6.2 | MIT | development-only-not-shipped |
| npm | lru-cache | 10.4.3 | ISC | development-only-not-shipped |
| npm | lucide-react | 0.468.0 | ISC | frontend-production |
| npm | lz-string | 1.5.0 | MIT | development-only-not-shipped |
| npm | magic-string | 1.4.4 | MIT | development-only-not-shipped |
| npm | min-indent | 1.0.1 | MIT | development-only-not-shipped |
| npm | minimatch | 3.1.5 | ISC | development-only-not-shipped |
| npm | ms | 2.1.3 | MIT | development-only-not-shipped |
| npm | nanoid | 3.3.20 | MIT | development-only-not-shipped |
| npm | natural-compare | 1.4.0 | MIT | development-only-not-shipped |
| npm | nwsapi | 2.2.28 | MIT | development-only-not-shipped |
| npm | obug | 3.0.0 | MIT | development-only-not-shipped |
| npm | optionator | 0.9.4 | MIT | development-only-not-shipped |
| npm | p-limit | 3.1.0 | MIT | development-only-not-shipped |
| npm | p-locate | 5.0.0 | MIT | development-only-not-shipped |
| npm | parent-module | 1.0.1 | MIT | development-only-not-shipped |
| npm | parse5 | 7.3.0 | MIT | development-only-not-shipped |
| npm | path-exists | 4.0.0 | MIT | development-only-not-shipped |
| npm | path-key | 3.1.1 | MIT | development-only-not-shipped |
| npm | picocolors | 1.1.1 | ISC | development-only-not-shipped |
| npm | picomatch | 4.0.7 | MIT | development-only-not-shipped |
| npm | postcss | 8.5.31 | MIT | development-only-not-shipped |
| npm | prelude-ls | 1.2.1 | MIT | development-only-not-shipped |
| npm | pretty-format | 27.5.1 | MIT | development-only-not-shipped |
| npm | ansi-styles | 5.2.0 | MIT | development-only-not-shipped |
| npm | punycode | 2.3.1 | MIT | development-only-not-shipped |
| npm | react | 19.3.1 | MIT | frontend-production |
| npm | react-dom | 19.3.1 | MIT | frontend-production |
| npm | react-is | 17.0.2 | MIT | development-only-not-shipped |
| npm | redent | 3.0.0 | MIT | development-only-not-shipped |
| npm | resolve-from | 4.0.0 | MIT | development-only-not-shipped |
| npm | rolldown | 1.2.15 | MIT | development-only-not-shipped |
| npm | rrweb-cssom | 0.8.0 | MIT | development-only-not-shipped |
| npm | safer-buffer | 2.1.2 | MIT | development-only-not-shipped |
| npm | saxes | 6.0.0 | ISC | development-only-not-shipped |
| npm | scheduler | 0.28.0 | MIT | frontend-production |
| npm | semver | 7.8.5 | ISC | development-only-not-shipped |
| npm | shebang-command | 2.0.0 | MIT | development-only-not-shipped |
| npm | shebang-regex | 3.0.0 | MIT | development-only-not-shipped |
| npm | source-map-js | 1.2.2 | BSD-3-Clause | development-only-not-shipped |
| npm | std-env | 4.3.0 | MIT | development-only-not-shipped |
| npm | strip-indent | 3.0.0 | MIT | development-only-not-shipped |
| npm | strip-json-comments | 3.1.1 | MIT | development-only-not-shipped |
| npm | supports-color | 7.2.0 | MIT | development-only-not-shipped |
| npm | symbol-tree | 3.2.4 | MIT | development-only-not-shipped |
| npm | tinybench | 6.2.2 | MIT | development-only-not-shipped |
| npm | tinyexec | 1.3.1 | MIT | development-only-not-shipped |
| npm | tinyglobby | 0.2.17 | MIT | development-only-not-shipped |
| npm | tldts | 6.1.86 | MIT | development-only-not-shipped |
| npm | tldts-core | 6.1.86 | MIT | development-only-not-shipped |
| npm | tough-cookie | 5.1.2 | BSD-3-Clause | development-only-not-shipped |
| npm | tr46 | 5.1.1 | MIT | development-only-not-shipped |
| npm | ts-api-utils | 2.5.0 | MIT | development-only-not-shipped |
| npm | type-check | 0.4.0 | MIT | development-only-not-shipped |
| npm | typescript | 5.9.3 | Apache-2.0 | development-only-not-shipped |
| npm | typescript-eslint | 8.72.0 | MIT | development-only-not-shipped |
| npm | undici-types | 6.21.0 | MIT | development-only-not-shipped |
| npm | uri-js | 4.4.1 | BSD-2-Clause | development-only-not-shipped |
| npm | vite | 8.3.5 | MIT | development-only-not-shipped |
| npm | vitest | 5.0.5 | MIT | development-only-not-shipped |
| npm | w3c-xmlserializer | 5.0.0 | MIT | development-only-not-shipped |
| npm | webidl-conversions | 7.0.0 | BSD-2-Clause | development-only-not-shipped |
| npm | whatwg-encoding | 3.1.1 | MIT | development-only-not-shipped |
| npm | whatwg-mimetype | 4.0.0 | MIT | development-only-not-shipped |
| npm | whatwg-url | 14.2.0 | MIT | development-only-not-shipped |
| npm | which | 2.0.2 | ISC | development-only-not-shipped |
| npm | why-is-node-running | 3.2.1 | MIT | development-only-not-shipped |
| npm | word-wrap | 1.2.5 | MIT | development-only-not-shipped |
| npm | ws | 8.22.0 | MIT | development-only-not-shipped |
| npm | xml-name-validator | 5.0.0 | Apache-2.0 | development-only-not-shipped |
| npm | xmlchars | 2.2.0 | MIT | development-only-not-shipped |
| npm | yocto-queue | 0.1.0 | MIT | development-only-not-shipped |
