<h1 align="center">R2Share</h1>

<p align="center">
  小さなWindows用exeにファイルをドラッグします。R2ShareがCloudflare R2へアップロードし、公開URLを表示して、クリップボードにも入れます。
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a>
</p>

<p align="center">
  <img alt="Rust 2024" src="https://img.shields.io/badge/Rust-2024-orange?logo=rust">
  <img alt="Windows 11" src="https://img.shields.io/badge/platform-Windows%2011-blue">
  <img alt="Cloudflare R2" src="https://img.shields.io/badge/storage-Cloudflare%20R2-f38020?logo=cloudflare">
  <img alt="MIT license" src="https://img.shields.io/badge/license-MIT-green">
</p>

> [!NOTE]
> このリポジトリのコードとドキュメントは、すべてOpenCodeによって作成されています。

R2Shareは、「このローカルファイルを今すぐ共有したい」時のための小さなツールです。ダッシュボードもアップロード画面もありません。`R2Share.exe`にファイルを放り込んで、アップロードが終わったらURLを貼る。それだけです。

## できること

| 機能 | 動き |
| --- | --- |
| ファイルアップロード | 1つ以上のローカルファイルをCloudflare R2へ送ります。 |
| 公開URLの生成 | `public_base_url`と保存先のobject keyからURLを作ります。 |
| クリップボードコピー | アップロードに成功したURLをまとめてコピーします。 |
| 複数ファイル対応 | 複数ファイルを順番に処理し、失敗したファイルは別で表示します。 |
| メタデータ設定 | `Content-Type`を判定し、`Content-Disposition`とキャッシュヘッダーを設定します。 |
| 一意なファイル名 | 日付パスとULIDで保存名を作ります。 |
| Windowsで見やすい終了 | Enterを押すまで閉じないので、ドラッグ＆ドロップでも結果を読めます。 |

## 処理の流れ

```mermaid
flowchart LR
    A[ファイルパスまたはドラッグ＆ドロップ] --> B[R2Share.exe]
    B --> C[r2share.tomlを読む]
    C --> D[Cloudflare R2へアップロード]
    D --> E[公開URLを作る]
    E --> F[URLをクリップボードへコピー]
```

## 動作に必要なもの

| 必要なもの | メモ |
| --- | --- |
| Windows 11 | 主にWindowsのドラッグ＆ドロップ利用を想定しています。 |
| Rust | ソースからビルドする時に必要です。 |
| Cloudflare R2 | バケット、アクセスキー、S3互換エンドポイントを使います。 |
| Public Bucketまたは公開ドメイン | `public_base_url`経由でアップロード済みobjectにアクセスできる前提です。 |

## 設定

`r2share.toml.example`を`r2share.toml`へコピーして、R2の値を入れます。

```toml
bucket = "discord-files"
endpoint = "https://<account_id>.r2.cloudflarestorage.com"
access_key_id = "<access_key_id>"
secret_access_key = "<secret_access_key>"
public_base_url = "https://files.example.com"
default_prefix = "uploads"
```

設定ファイルはこの順で探します。

1. `R2Share.exe`と同じディレクトリの`r2share.toml`
2. `%APPDATA%\R2Share\config.toml`

配布用exeなら、`R2Share.exe`と同じフォルダに`r2share.toml`を置くのが一番楽です。開発中に`cargo run`で試すなら、`%APPDATA%\R2Share\config.toml`のほうが扱いやすいです。

## ビルド

```powershell
cargo build --release
```

生成されるexe:

```text
target\release\R2Share.exe
```

## 使い方

1ファイルだけアップロード:

```powershell
R2Share.exe C:\path\to\video.mp4
```

複数ファイルをアップロード:

```powershell
R2Share.exe C:\path\to\image.png C:\path\to\archive.zip
```

Explorer上でファイルを`R2Share.exe`にドラッグ＆ドロップしても使えます。Windowsがファイルパスを引数として渡すので、CLI実行と同じ処理になります。

すべて成功した場合、生成されたURLは改行区切りでクリップボードに入ります。一部だけ失敗した場合も、成功したURLは表示され、失敗したファイルは別に出ます。

## object keyとURL

アップロードされたファイルは、この形のobject keyで保存されます。

```text
uploads/YYYY/MM/DD/<ULID>.<ext>
```

例:

```text
uploads/2026/05/03/01KQQ13CPYWS6AYYVVCRZ0N2CK.mp4
```

公開URLは`public_base_url`とobject keyをつなげたものです。

```text
https://files.example.com/uploads/2026/05/03/01KQQ13CPYWS6AYYVVCRZ0N2CK.mp4
```

## アップロード時のヘッダー

| ヘッダー | 値 |
| --- | --- |
| `Content-Type` | 拡張子から判定します。わからない場合は`application/octet-stream`です。 |
| `Content-Disposition` | 画像、動画、音声、テキスト、JSON、XML、PDFは`inline`。それ以外は`attachment`です。 |
| `Cache-Control` | `public, max-age=31536000` |

## 開発用コマンド

```powershell
cargo fmt
cargo test
cargo run -- C:\path\to\file.mp4
```

## 注意

R2Shareは公開ファイル用です。生成されたURLを知っている人は、そのファイルを開けます。秘密情報、個人文書、認証が必要なファイルには使わないでください。

本物の設定ファイルには`secret_access_key`が入ります。`r2share.toml`はGitに入れないでください。このリポジトリではすでにignoreしています。

## 実装状況

| 実装済み | 未実装 |
| --- | --- |
| `r2share.toml`の読み込み | Windows通知 |
| 1つ以上のファイルアップロード | アップロード履歴 |
| ULIDベースのobject key生成 | URL削除機能 |
| コンテンツヘッダー設定 | prefixのCLI指定 |
| 公開URLの表示とコピー | GUI |
