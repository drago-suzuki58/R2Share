# R2Share

R2Share は、Cloudflare R2 にファイルをアップロードして公開 URL を取得する小さな Rust 製ツールです。

Windows では `R2Share.exe` にファイルをドラッグ＆ドロップするだけで使えます。アップロード後、生成された公開 URL はコンソールに表示され、クリップボードにもコピーされます。

## できること

- Cloudflare R2 へのファイルアップロード
- 公開 URL の生成
- クリップボードへの URL コピー
- 複数ファイルの順次アップロード
- MIME Type の自動判定
- ULID を使った一意なファイル名生成
- アップロード中の進捗バー表示
- 終了前の Enter 待ち

## 動作環境

- Windows 11 を主対象
- Rust
- Cloudflare R2
- Public Bucket と、そのバケットに向いた公開ドメイン

## セットアップ

### 1. 設定ファイルを用意する

`r2share.toml.example` を参考に、`r2share.toml` を作成します。

```toml
bucket = "discord-files"
endpoint = "https://YOUR_ACCOUNT_ID.r2.cloudflarestorage.com"
access_key_id = "YOUR_ACCESS_KEY_ID"
secret_access_key = "YOUR_SECRET_ACCESS_KEY"
public_base_url = "https://files.example.com"
default_prefix = "uploads"
```

設定ファイルの探索順:

1. 実行ファイルと同じディレクトリの `r2share.toml`
2. `%APPDATA%\R2Share\config.toml`

通常の配布用 exe を使う場合は、`R2Share.exe` と同じフォルダに `r2share.toml` を置くのが簡単です。

`cargo run` で開発中に試す場合は、`%APPDATA%\R2Share\config.toml` を使うと扱いやすいです。

### 2. ビルドする

```powershell
cargo build --release
```

生成物:

```txt
target\release\R2Share.exe
```

## 使い方

### 単一ファイルをアップロードする

```powershell
R2Share.exe C:\path\to\video.mp4
```

### 複数ファイルをアップロードする

```powershell
R2Share.exe C:\path\to\image.png C:\path\to\archive.zip
```

全件成功した場合は、生成された URL が改行区切りでクリップボードに入ります。

### ドラッグ＆ドロップで使う

Windows では、ファイルを `R2Share.exe` にドラッグ＆ドロップすると、そのファイルパスが引数として渡されます。

そのため、CLI 実行と同じ処理でそのまま動きます。

アップロード中は進捗バーが表示されます。処理完了後はウィンドウがすぐ閉じないように、`Press Enter to exit...` が表示され、Enter を押すまで終了しません。

## URL 仕様

アップロードされたファイルは、以下の形式の object key で保存されます。

```txt
uploads/YYYY/MM/DD/<ULID>.<ext>
```

例:

```txt
uploads/2026/05/03/01KQQ13CPYWS6AYYVVCRZ0N2CK.mp4
```

公開 URL は `public_base_url` と object key を連結して生成します。

例:

```txt
https://files.example.com/uploads/2026/05/03/01KQQ13CPYWS6AYYVVCRZ0N2CK.mp4
```

## メタデータ

アップロード時に、以下を設定します。

- `Content-Type`: 拡張子から自動判定。判定できない場合は `application/octet-stream`
- `Content-Disposition`: 画像、動画、音声、テキスト、PDF などは `inline`。それ以外は `attachment`
- `Cache-Control`: `public, max-age=31536000`

## エラー時の挙動

- アップロード失敗時は、理由をコンソールに表示します
- クリップボードへのコピーに失敗しても、URL 自体はコンソールに表示されます
- 一部のファイルだけ失敗した場合は、成功した URL と失敗したファイルをそれぞれ表示します

## 開発用コマンド

```powershell
cargo fmt
cargo test
cargo run -- C:\path\to\file.mp4
```

## 注意事項

- アップロード先は Public Bucket 想定です
- URL を知っていれば誰でもアクセスできます
- 機密ファイルのアップロードは想定していません
- `secret_access_key` を含む設定ファイルは Git 管理しないでください

## 現在の実装範囲

現時点で実装済み:

- `r2share.toml` の読み込み
- 単一ファイルアップロード
- 複数ファイルアップロード
- ULID 付きファイル名生成
- Content-Type 自動設定
- Public URL 生成
- クリップボードコピー
- コンソール表示

未実装:

- Windows 通知
- アップロード履歴
- URL 削除機能
- prefix の CLI 指定
- GUI
