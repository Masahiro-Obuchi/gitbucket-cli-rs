# gb — GitBucket CLI

`gb` は GitBucket をコマンドラインから操作するための CLI ツールです。  
[GitHub CLI (`gh`)](https://cli.github.com/) の設計思想を参考に、Rust で実装されています。

```
$ gb issue list
#    STATE   TITLE                          AUTHOR   LABELS
#1   OPEN    Fix login page bug             alice    bug
#3   OPEN    Add dark mode support          bob      enhancement

$ gb pr create -t "Add feature X" -H feature/x -B main
✓ Created pull request #5: Add feature X
```

## インストール

### ソースからビルド

```bash
git clone https://github.com/your-org/gitbucket-cli-rs.git
cd gitbucket-cli-rs
cargo build --release
cp target/release/gb ~/.local/bin/
```

**前提条件:** Rust 1.70 以上、`git` コマンド

## クイックスタート

### 1. 認証

```bash
gb auth login
# GitBucket hostname: gitbucket.example.com
# Personal access token: ********
# ✓ Logged in to gitbucket.example.com as alice
```

GitBucket の **Account Settings → Personal access tokens** からトークンを発行してください。

### 2. リポジトリ操作

```bash
gb repo list                        # リポジトリ一覧
gb repo view alice/my-app           # 詳細表示
gb repo create my-new-repo          # 新規作成
gb repo clone alice/my-app          # クローン
gb repo fork alice/my-app           # フォーク
```

### 3. Issue 操作

```bash
gb issue list                       # Issue 一覧
gb issue create -t "Bug report"     # 作成
gb issue view 1                     # 詳細表示
gb issue close 1                    # クローズ
gb issue comment 1 -b "Fixed!"     # コメント追加
```

### 4. プルリクエスト操作

```bash
gb pr list                          # PR 一覧
gb pr create                        # 作成 (現在のブランチから)
gb pr view 5                        # 詳細表示
gb pr merge 5                       # マージ
gb pr checkout 5                    # ブランチをチェックアウト
gb pr diff 5                        # 差分表示
```

## コマンド一覧

| コマンド | 説明 |
|---------|------|
| `gb auth login` | GitBucket インスタンスへ認証 |
| `gb auth logout` | ログアウト |
| `gb auth status` | 認証状態の確認 |
| `gb auth token` | アクセストークンを表示 |
| `gb repo list [OWNER]` | リポジトリ一覧 |
| `gb repo view [OWNER/REPO]` | リポジトリ詳細 |
| `gb repo create [NAME]` | リポジトリ作成 |
| `gb repo clone <REPO>` | リポジトリのクローン |
| `gb repo delete [OWNER/REPO]` | リポジトリ削除 |
| `gb repo fork [OWNER/REPO]` | リポジトリのフォーク |
| `gb issue list` | Issue 一覧 |
| `gb issue view <NUMBER>` | Issue 詳細 |
| `gb issue create` | Issue 作成 |
| `gb issue close <NUMBER>` | Issue クローズ |
| `gb issue reopen <NUMBER>` | Issue 再オープン |
| `gb issue comment <NUMBER>` | Issue にコメント追加 |
| `gb pr list` | PR 一覧 |
| `gb pr view <NUMBER>` | PR 詳細 |
| `gb pr create` | PR 作成 |
| `gb pr close <NUMBER>` | PR クローズ |
| `gb pr merge <NUMBER>` | PR マージ |
| `gb pr checkout <NUMBER>` | PR ブランチをチェックアウト |
| `gb pr diff <NUMBER>` | PR 差分表示 |
| `gb pr comment <NUMBER>` | PR にコメント追加 |
| `gb browse` | リポジトリをブラウザで開く |

## グローバルオプション

```
-H, --hostname <HOST>    GitBucket ホスト名
-R, --repo <OWNER/REPO>  操作対象リポジトリ
-h, --help               ヘルプ表示
-V, --version            バージョン表示
```

## リポジトリの自動推定

`-R` オプションを省略した場合、カレントディレクトリの git remote (`origin`) から自動的にオーナー/リポジトリ名を推定します。

```bash
cd ~/projects/my-app    # git remote = https://gitbucket.example.com/alice/my-app.git
gb issue list           # → alice/my-app の Issue を表示
```

## 出力形式

```bash
gb issue list              # テーブル形式 (デフォルト)
gb issue list --json       # JSON 形式
gb issue view 1 --web      # ブラウザで開く
```

## 環境変数

| 変数名 | 説明 |
|-------|------|
| `GB_TOKEN` | アクセストークン (設定ファイルより優先) |
| `GB_HOST` | デフォルトの GitBucket ホスト名 |
| `GB_REPO` | デフォルトのリポジトリ (`OWNER/REPO`) |
| `GB_CONFIG_DIR` | 設定ディレクトリのパス (デフォルト: `~/.config/gb/`) |
| `NO_COLOR` | カラー出力の無効化 |

## 設定ファイル

認証情報は `~/.config/gb/config.toml` に保存されます。

```toml
[hosts."gitbucket.example.com"]
token = "your-personal-access-token"
user = "alice"
protocol = "https"
```

## ライセンス

MIT
