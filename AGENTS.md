# ローカルでの動作確認

UI の調整・デバッグ・試作品の動作確認にはデバッグビルドを使う。利用者に渡して触ってもらう場合も、通常はデバッグ版を先に起動する。UI の確認結果はデバッグ版で確認した時点で返し、配布用 CI の完了待ちと分ける。

Windows 版は `scripts/windows.ps1 -Run` でデバッグビルドして起動する。リリースビルド（`-Release`）は、配布・正式リリース・最適化後の検証が必要なときに使う。Rust テストも実行する場合は `-Test` を付ける。

ビルドと操作確認の詳細は [docs/desktop.md](docs/desktop.md) を参照する。

# 変更後の検査

- HTML・JavaScript の変更後は `pnpm check` と `pnpm test` を実行する。
- Rust の変更後は `cargo fmt --manifest-path src-tauri/Cargo.toml --all` で整形し、`pnpm format:check` で確認する。動作を変えた場合は Windows で関連テストも実行する。
- pre-commit hook の導入は [docs/desktop.md](docs/desktop.md#pre-commit-hook) を参照する。
