# GitBucket CLI (`gb`) 仕様書

## 1. 概要

`gb` は GitBucket をコマンドラインから操作するための CLI ツールである。
GitHub CLI (`gh`) の設計思想を参考に、GitBucket の REST API を通じてリポジトリ・Issue・プルリクエスト等を管理する。

- **コマンド名**: `gb`
- **プログラミング言語**: Rust
- **対象プラットフォーム**: Linux / macOS / Windows
- **ライセンス**: MIT

---

## 2. 設計方針

### 2.1 GitHub CLI (`gh`) との対応関係

| GitHub CLI | GitBucket CLI | 対応状況 | 備考 |
|-----------|--------------|---------|------|
| `gh auth` | `gb auth` | ✅ 実装済 | Personal Access Token のみ対応 |
| `gh repo` | `gb repo` | ✅ 実装済 | |
| `gh issue` | `gb issue` | ✅ 実装済 | |
| `gh pr` | `gb pr` | ✅ 実装済 | |
| `gh browse` | `gb browse` | ✅ 実装済 | |
| `gh label` | `gb label` | 📋 計画中 | Phase 2 |
| `gh release` | `gb release` | 📋 計画中 | Phase 2 (API対応状況による) |
| `gh org` | `gb org` | 📋 計画中 | Phase 2 |
| `gh api` | `gb api` | 📋 計画中 | Phase 3 |
| `gh config` | `gb config` | 📋 計画中 | Phase 3 |
| `gh completion` | `gb completion` | 📋 計画中 | Phase 3 |
| `gh codespace` | — | ❌ 対象外 | GitBucket に該当機能なし |
| `gh gist` | — | ❌ 対象外 | GitBucket に該当機能なし |
| `gh project` | — | ❌ 対象外 | GitBucket に該当機能なし |
| `gh run` / `gh workflow` / `gh cache` | — | ❌ 対象外 | GitBucket に CI/CD 機能なし |
| `gh secret` / `gh variable` | — | ❌ 対象外 | Actions 連携機能なし |
| `gh ssh-key` / `gh gpg-key` | — | ❌ 対象外 | 該当 API なし |
| `gh ruleset` | — | ❌ 対象外 | GitBucket にルールセット機能なし |
| `gh search` | — | ❌ 対象外 | GitBucket に汎用検索 API なし |
| `gh attestation` | — | ❌ 対象外 | GitBucket にアテステーション機能なし |

### 2.2 GitBucket 固有の機能

GitHub CLI には無いが、GitBucket API で利用可能な機能:

| コマンド | 説明 | 対応状況 |
|---------|------|---------|
| `gb milestone` | マイルストーン管理 | 📋 計画中 (Phase 2) |
| `gb user` | ユーザー管理 (管理者向け) | 📋 計画中 (Phase 2) |
| `gb webhook` | Webhook 管理 | 📋 計画中 (Phase 3) |
| `gb repo collaborator` | コラボレーター管理 | 📋 計画中 (Phase 3) |

---

## 3. コマンドリファレンス

### 3.1 グローバルオプション

すべてのコマンドで使用可能なオプション:

| オプション | 短縮形 | 環境変数 | 説明 |
|-----------|-------|---------|------|
| `--hostname <HOST>` | `-H` | `GB_HOST` | 接続先 GitBucket インスタンスのホスト名 |
| `--repo <OWNER/REPO>` | `-R` | `GB_REPO` | 操作対象のリポジトリ (`OWNER/REPO` 形式) |
| `--help` | `-h` | — | ヘルプメッセージの表示 |
| `--version` | `-V` | — | バージョンの表示 |

### 3.2 `gb auth` — 認証管理

GitBucket インスタンスへの認証を管理する。

#### `gb auth login`

GitBucket インスタンスへ認証する。

```
gb auth login [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--hostname <HOST>` | `-H` | (対話入力) | GitBucket のホスト名 (例: `gitbucket.example.com`) |
| `--token <TOKEN>` | `-t` | (対話入力) | Personal Access Token |
| `--protocol <PROTOCOL>` | — | `https` | 通信プロトコル (`https` または `http`) |

**動作仕様:**

1. ホスト名が未指定の場合、対話的に入力を求める
2. トークンが未指定の場合、パスワード形式 (非表示) で入力を求める
3. 指定されたトークンで `GET /api/v3/user` を呼び出し、認証の有効性を検証する
4. 認証成功時、設定ファイルにホスト情報を保存する
5. 認証失敗時、エラーメッセージを表示する

**使用例:**

```bash
# 対話的にログイン
gb auth login

# オプション指定でログイン
gb auth login -H gitbucket.example.com -t ghp_xxxxxxxxxxxx

# HTTP プロトコルを使用
gb auth login -H localhost:8080 --protocol http
```

#### `gb auth logout`

保存された認証情報を削除する。

```
gb auth logout [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--hostname <HOST>` | `-H` | (デフォルトホスト) | ログアウト対象のホスト名 |

#### `gb auth status`

現在の認証状態を表示する。

```
gb auth status
```

**出力例:**

```
gitbucket.example.com
  ✓ Logged in as alice
  Protocol: https
```

#### `gb auth token`

指定したホストのアクセストークンを標準出力に出力する。スクリプト連携用。

```
gb auth token [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--hostname <HOST>` | `-H` | (デフォルトホスト) | トークンを取得するホスト名 |

---

### 3.3 `gb repo` — リポジトリ管理

#### `gb repo list`

リポジトリの一覧を表示する。

```
gb repo list [OWNER] [OPTIONS]
```

| 引数/オプション | 説明 |
|---------------|------|
| `OWNER` | ユーザー名または Organization 名 (省略時: 認証ユーザーのリポジトリ) |
| `--json` | JSON 形式で出力 |

**出力 (テーブル形式):**

```
NAME               DESCRIPTION                 VISIBILITY
alice/my-app       My awesome application       public
alice/private-lib  Internal library             private
```

#### `gb repo view`

リポジトリの詳細を表示する。

```
gb repo view [OWNER/REPO] [OPTIONS]
```

| 引数/オプション | 短縮形 | 説明 |
|---------------|-------|------|
| `OWNER/REPO` | — | リポジトリ (省略時: git remote から自動推定) |
| `--web` | `-w` | ブラウザで開く |

**出力例:**

```
alice/my-app
My awesome application

Visibility: Public  Default branch: main
URL: https://gitbucket.example.com/alice/my-app
Clone: https://gitbucket.example.com/git/alice/my-app.git

Stars: 12  Forks: 3  Issues: 5
```

#### `gb repo create`

新しいリポジトリを作成する。

```
gb repo create [NAME] [OPTIONS]
```

| 引数/オプション | 短縮形 | デフォルト | 説明 |
|---------------|-------|----------|------|
| `NAME` | — | (対話入力) | リポジトリ名 |
| `--description <DESC>` | `-d` | — | リポジトリの説明 |
| `--private` | — | `false` | プライベートリポジトリとして作成 |
| `--add-readme` | — | `false` | README.md を自動生成 |
| `--org <ORG>` | — | — | Organization 配下に作成 |

**使用例:**

```bash
# 対話的に作成
gb repo create

# オプション指定で作成
gb repo create my-new-repo -d "A new project" --private --add-readme

# Organization 配下に作成
gb repo create team-tool --org my-org
```

#### `gb repo clone`

リポジトリをローカルにクローンする。内部で `git clone` を実行する。

```
gb repo clone <REPO> [DIRECTORY]
```

| 引数 | 説明 |
|-----|------|
| `REPO` | `OWNER/REPO` 形式、または完全な URL |
| `DIRECTORY` | クローン先ディレクトリ (省略時: リポジトリ名) |

**使用例:**

```bash
gb repo clone alice/my-app
gb repo clone alice/my-app ./my-local-dir
gb repo clone https://gitbucket.example.com/git/alice/my-app.git
```

#### `gb repo delete`

リポジトリを削除する。

```
gb repo delete [OWNER/REPO] [OPTIONS]
```

| 引数/オプション | 説明 |
|---------------|------|
| `OWNER/REPO` | 削除対象 (省略時: git remote から自動推定) |
| `--yes` | 確認プロンプトをスキップ |

**動作仕様:**

- `--yes` が指定されていない場合、削除確認のプロンプトを表示する
- デフォルトは「いいえ」

#### `gb repo fork`

リポジトリをフォークする。

```
gb repo fork [OWNER/REPO]
```

| 引数 | 説明 |
|-----|------|
| `OWNER/REPO` | フォーク対象 (省略時: git remote から自動推定) |

---

### 3.4 `gb issue` — Issue 管理

#### `gb issue list`

Issue の一覧を表示する。

```
gb issue list [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--state <STATE>` | `-s` | `open` | フィルタ (`open`, `closed`, `all`) |
| `--json` | — | — | JSON 形式で出力 |

**出力 (テーブル形式):**

```
#    STATE   TITLE                          AUTHOR   LABELS
#1   OPEN    Fix login page bug             alice    bug
#3   OPEN    Add dark mode support          bob      enhancement
```

#### `gb issue view`

Issue の詳細を表示する。

```
gb issue view <NUMBER> [OPTIONS]
```

| 引数/オプション | 短縮形 | 説明 |
|---------------|-------|------|
| `NUMBER` | — | Issue 番号 |
| `--comments` | `-c` | コメントも表示 |
| `--web` | `-w` | ブラウザで開く |

**出力例:**

```
Fix login page bug #1
OPEN

Author: alice  Created: 2025-01-15T10:30:00Z
Labels: bug

Login page returns 500 error when password contains special characters.
```

#### `gb issue create`

新しい Issue を作成する。

```
gb issue create [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--title <TITLE>` | `-t` | (対話入力) | Issue のタイトル |
| `--body <BODY>` | `-b` | (対話入力) | Issue の本文 |
| `--label <LABEL>` | `-l` | — | ラベル (複数指定可) |
| `--assignee <USER>` | `-a` | — | 担当者 (複数指定可) |

**使用例:**

```bash
# 対話的に作成
gb issue create

# オプション指定で作成
gb issue create -t "Fix bug" -b "Description here" -l bug -l urgent
```

#### `gb issue close`

Issue をクローズする。

```
gb issue close <NUMBER>
```

**動作仕様:** GitBucket API の `PATCH /repos/:owner/:repo/issues/:number` で `state: "closed"` を送信する。

#### `gb issue reopen`

クローズされた Issue を再オープンする。

```
gb issue reopen <NUMBER>
```

#### `gb issue comment`

Issue にコメントを追加する。

```
gb issue comment <NUMBER> [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--body <BODY>` | `-b` | (対話入力) | コメント本文 |

---

### 3.5 `gb pr` — プルリクエスト管理

#### `gb pr list`

プルリクエストの一覧を表示する。

```
gb pr list [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--state <STATE>` | `-s` | `open` | フィルタ (`open`, `closed`, `all`) |
| `--json` | — | — | JSON 形式で出力 |

**出力 (テーブル形式):**

```
#    STATE    TITLE                       BRANCH            AUTHOR
#5   OPEN     Add user profile page       feature/profile   alice
#4   MERGED   Fix database migration      fix/db-migrate    bob
```

#### `gb pr view`

プルリクエストの詳細を表示する。

```
gb pr view <NUMBER> [OPTIONS]
```

| 引数/オプション | 短縮形 | 説明 |
|---------------|-------|------|
| `NUMBER` | — | PR 番号 |
| `--comments` | `-c` | コメントも表示 |
| `--web` | `-w` | ブラウザで開く |

**出力例:**

```
Add user profile page #5
OPEN

main ← feature/profile
Author: alice  Created: 2025-02-01T09:00:00Z

Implements the user profile page with avatar upload.
```

#### `gb pr create`

新しいプルリクエストを作成する。

```
gb pr create [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--title <TITLE>` | `-t` | (対話入力) | PR のタイトル |
| `--body <BODY>` | `-b` | (対話入力) | PR の本文 |
| `--head <BRANCH>` | `-H` | (現在のブランチ) | マージ元ブランチ |
| `--base <BRANCH>` | `-B` | `main` (対話入力) | マージ先ブランチ |

**動作仕様:**

1. `--head` 未指定時、`git branch --show-current` で現在のブランチを自動検出する
2. 検出できない場合、対話的に入力を求める
3. `--base` 未指定時、対話的に入力を求める (デフォルト: `main`)

**使用例:**

```bash
# 現在のブランチから main へのPRを対話的に作成
gb pr create

# オプション指定で作成
gb pr create -t "Add feature X" -b "Details..." -H feature/x -B develop
```

#### `gb pr close`

プルリクエストをクローズする。

```
gb pr close <NUMBER>
```

#### `gb pr merge`

プルリクエストをマージする。

```
gb pr merge <NUMBER> [OPTIONS]
```

| オプション | 短縮形 | 説明 |
|-----------|-------|------|
| `--message <MSG>` | `-m` | マージコミットメッセージ |

**動作仕様:** `PUT /repos/:owner/:repo/pulls/:number/merge` を呼び出す。マージ成功時は `✓ Merged pull request #N` と表示し、失敗時はエラーメッセージを表示する。

#### `gb pr checkout`

プルリクエストのブランチをローカルにチェックアウトする。

```
gb pr checkout <NUMBER>
```

**動作仕様:**

1. `GET /repos/:owner/:repo/pulls/:number` で PR の head ブランチ名を取得する
2. `git fetch origin <branch>` でブランチをフェッチする
3. `git checkout <branch>` でブランチに切り替える

#### `gb pr diff`

プルリクエストの差分を表示する。

```
gb pr diff <NUMBER>
```

**動作仕様:**

1. PR の head / base ブランチ名を API から取得する
2. `git fetch origin <head> <base>` で両ブランチをフェッチする
3. `git diff origin/<base>...origin/<head>` で差分を表示する

#### `gb pr comment`

プルリクエストにコメントを追加する。

```
gb pr comment <NUMBER> [OPTIONS]
```

| オプション | 短縮形 | デフォルト | 説明 |
|-----------|-------|----------|------|
| `--body <BODY>` | `-b` | (対話入力) | コメント本文 |

---

### 3.6 `gb browse` — ブラウザで開く

現在のリポジトリを Web ブラウザで開く。

```
gb browse
```

**動作仕様:** リポジトリの Web URL (`<protocol>://<hostname>/<owner>/<repo>`) を構築し、`open` クレートでデフォルトブラウザを起動する。

---

## 4. 認証

### 4.1 認証方式

**Personal Access Token (PAT)** のみをサポートする。

- HTTP ヘッダー: `Authorization: token <TOKEN>`
- GitBucket の設定画面 → Account Settings → Personal access tokens からトークンを発行する

### 4.2 認証情報の保存

設定ファイルパス: `~/.config/gb/config.toml`

```toml
[hosts]

[hosts."gitbucket.example.com"]
token = "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
user = "alice"
protocol = "https"

[hosts."gitbucket-staging.example.com"]
token = "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy"
user = "alice"
protocol = "http"
```

**複数ホスト対応:** 異なる GitBucket インスタンスに対して個別に認証情報を保存できる。

### 4.3 認証の優先順位

以下の優先順位でトークンを解決する:

1. 環境変数 `GB_TOKEN` (最優先)
2. 設定ファイル `~/.config/gb/config.toml` 内の該当ホスト情報

### 4.4 ホスト名の解決順序

1. `--hostname` (`-H`) オプション
2. 環境変数 `GB_HOST`
3. 設定ファイル内の最初のホスト

---

## 5. リポジトリの自動推定

`--repo` (`-R`) オプションが未指定の場合、カレントディレクトリの git remote から自動的にオーナー名/リポジトリ名を推定する。

### 5.1 対応する URL 形式

| 形式 | 例 |
|-----|-----|
| HTTPS | `https://gitbucket.example.com/alice/my-repo.git` |
| SSH | `git@gitbucket.example.com:alice/my-repo.git` |
| GitBucket Git URL | `https://gitbucket.example.com/git/alice/my-repo.git` |

### 5.2 解決順序

1. `--repo` (`-R`) オプション
2. 環境変数 `GB_REPO`
3. `git remote get-url origin` の出力を解析

---

## 6. 出力形式

### 6.1 テーブル形式 (デフォルト)

一覧系コマンドではカラム幅が自動調整されたテーブルを表示する。

- ヘッダー行はグレーで表示
- ステータスは色分け: `OPEN` (緑)、`CLOSED` (赤)、`MERGED` (マゼンタ)
- リポジトリの公開状態: `public` (緑)、`private` (黄)
- 長い文字列は自動的に省略される (末尾 `...`)

### 6.2 JSON 形式

`--json` フラグを指定すると、API レスポンスを整形済み JSON として出力する。スクリプトやパイプラインでの利用に適している。

```bash
gb repo list --json | jq '.[].full_name'
gb issue list --json | jq '.[] | select(.state == "open")'
```

### 6.3 ブラウザ表示

`--web` (`-w`) フラグを指定すると、該当リソースをデフォルト Web ブラウザで開く。

対応コマンド: `gb repo view --web`、`gb issue view --web`、`gb pr view --web`、`gb browse`

---

## 7. 環境変数

| 変数名 | 説明 | 使用例 |
|-------|------|-------|
| `GB_TOKEN` | アクセストークン (設定ファイルより優先) | `GB_TOKEN=xxx gb repo list` |
| `GB_HOST` | デフォルトの GitBucket ホスト名 | `export GB_HOST=gitbucket.example.com` |
| `GB_REPO` | デフォルトのリポジトリ | `export GB_REPO=alice/my-repo` |
| `GB_CONFIG_DIR` | 設定ディレクトリのパス | `export GB_CONFIG_DIR=/custom/path` |
| `NO_COLOR` | カラー出力の無効化 | `NO_COLOR=1 gb issue list` |

---

## 8. エラーハンドリング

### 8.1 エラー種別

| エラー種別 | メッセージ例 | 発生条件 |
|----------|-----------|---------|
| `Auth` | `Authentication error: ...` | トークン不正、認証失敗 |
| `Api` | `API error (404): Not Found` | API レスポンスがエラーステータス |
| `Config` | `Configuration error: ...` | 設定ファイルの読み書き失敗 |
| `NotAuthenticated` | `Not authenticated. Run \`gb auth login\` first.` | 未ログイン状態でコマンド実行 |
| `RepoNotFound` | `Repository not found. Specify with --repo ...` | リポジトリの自動推定に失敗 |
| `Http` | `HTTP error: ...` | ネットワーク障害、接続タイムアウト |
| `Io` | `IO error: ...` | ファイル I/O 失敗 |

### 8.2 終了コード

| コード | 意味 |
|-------|------|
| `0` | 正常終了 |
| `1` | エラー発生 |

---

## 9. 技術仕様

### 9.1 プロジェクト構成

```
gitbucket-cli-rs/
├── Cargo.toml
├── SPEC.md                  # 本仕様書
├── src/
│   ├── main.rs              # エントリーポイント (tokio::main)
│   ├── error.rs             # エラー型定義 (thiserror)
│   ├── cli/                 # CLI コマンド定義
│   │   ├── mod.rs           # Cli 構造体, Commands enum (clap)
│   │   ├── common.rs        # ホスト名/リポジトリ解決、クライアント生成
│   │   ├── auth.rs          # gb auth サブコマンド
│   │   ├── repo.rs          # gb repo サブコマンド
│   │   ├── issue.rs         # gb issue サブコマンド
│   │   └── pr.rs            # gb pr サブコマンド
│   ├── api/                 # GitBucket REST API クライアント
│   │   ├── mod.rs
│   │   ├── client.rs        # 汎用 HTTP クライアント (reqwest)
│   │   ├── repository.rs    # リポジトリ API メソッド
│   │   ├── issue.rs         # Issue API メソッド
│   │   └── pull_request.rs  # PR API メソッド
│   ├── models/              # データモデル (serde)
│   │   ├── mod.rs
│   │   ├── user.rs          # User 構造体
│   │   ├── repository.rs    # Repository, CreateRepository
│   │   ├── issue.rs         # Issue, CreateIssue, UpdateIssue, Label
│   │   ├── pull_request.rs  # PullRequest, CreatePullRequest, MergePullRequest
│   │   └── comment.rs       # Comment, CreateComment
│   ├── config/              # 設定管理
│   │   ├── mod.rs           # config_dir(), ensure_config_dir()
│   │   └── auth.rs          # AuthConfig, HostConfig (TOML)
│   └── output/              # 出力フォーマッタ
│       ├── mod.rs           # format_state(), truncate()
│       └── table.rs         # print_table() (カラム幅自動調整)
```

### 9.2 依存クレート

| クレート | バージョン | 用途 |
|---------|----------|------|
| `clap` | 4.x (derive, env) | CLI パーサー |
| `reqwest` | 0.12.x (json, rustls-tls) | HTTP クライアント |
| `tokio` | 1.x (full) | 非同期ランタイム |
| `serde` | 1.x (derive) | シリアライズ/デシリアライズ |
| `serde_json` | 1.x | JSON 処理 |
| `toml` | 0.8.x | 設定ファイル処理 |
| `dirs` | 6.x | OS 標準ディレクトリパス取得 |
| `thiserror` | 2.x | エラー型定義 |
| `anyhow` | 1.x | エラーハンドリング |
| `colored` | 3.x | ターミナル色付け |
| `dialoguer` | 0.11.x | 対話的プロンプト |
| `open` | 5.x | ブラウザ起動 |
| `url` | 2.x | URL パース |

### 9.3 API クライアント設計

`ApiClient` 構造体が GitBucket REST API (`/api/v3`) への全リクエストを処理する。

**ベース URL 構築:** `<protocol>://<hostname>/api/v3`

**サポートする HTTP メソッド:**

| メソッド | 用途 |
|---------|------|
| `GET` | リソース取得・一覧 |
| `POST` | リソース作成 |
| `PATCH` | リソース更新 |
| `PUT` | マージ操作等 |
| `DELETE` | リソース削除 |

**利用する GitBucket API エンドポイント:**

| エンドポイント | メソッド | 用途 |
|--------------|---------|------|
| `GET /user` | GET | 認証ユーザー情報取得 |
| `GET /user/repos` | GET | 認証ユーザーのリポジトリ一覧 |
| `GET /users/:owner/repos` | GET | 指定ユーザーのリポジトリ一覧 |
| `GET /repos/:owner/:repo` | GET | リポジトリ詳細 |
| `POST /user/repos` | POST | リポジトリ作成 (ユーザー) |
| `POST /orgs/:org/repos` | POST | リポジトリ作成 (Organization) |
| `DELETE /repos/:owner/:repo` | DELETE | リポジトリ削除 |
| `POST /repos/:owner/:repo/forks` | POST | フォーク |
| `GET /repos/:owner/:repo/issues` | GET | Issue 一覧 |
| `GET /repos/:owner/:repo/issues/:number` | GET | Issue 詳細 |
| `POST /repos/:owner/:repo/issues` | POST | Issue 作成 |
| `PATCH /repos/:owner/:repo/issues/:number` | PATCH | Issue 更新 (close/reopen) |
| `GET /repos/:owner/:repo/issues/:number/comments` | GET | Issue コメント一覧 |
| `POST /repos/:owner/:repo/issues/:number/comments` | POST | Issue コメント追加 |
| `GET /repos/:owner/:repo/pulls` | GET | PR 一覧 |
| `GET /repos/:owner/:repo/pulls/:number` | GET | PR 詳細 |
| `POST /repos/:owner/:repo/pulls` | POST | PR 作成 |
| `PUT /repos/:owner/:repo/pulls/:number/merge` | PUT | PR マージ |

---

## 10. 今後の拡張計画

### Phase 2: 拡張機能

| コマンド | サブコマンド | 説明 |
|---------|------------|------|
| `gb label` | `list`, `create`, `delete`, `edit` | ラベル管理 |
| `gb milestone` | `list`, `create`, `close`, `delete` | マイルストーン管理 |
| `gb release` | `list`, `view`, `create` | リリース管理 |
| `gb user` | `list`, `view` | ユーザー管理 |
| `gb org` | `list`, `view` | Organization 管理 |

### Phase 3: 高度な機能

| コマンド | サブコマンド | 説明 |
|---------|------------|------|
| `gb api` | — | 任意の API エンドポイントへの直接リクエスト |
| `gb config` | `get`, `set`, `list` | 設定管理 |
| `gb completion` | `bash`, `zsh`, `fish` | シェル補完スクリプト生成 |
| `gb repo collaborator` | `list`, `add`, `remove` | コラボレーター管理 |
| `gb webhook` | `list`, `create`, `delete` | Webhook 管理 |
| `gb status` | — | 自分に関連する Issue/PR のサマリー表示 |

### Phase 4: 品質・仕上げ

- 単体テスト・統合テストの拡充
- CI/CD パイプライン構築
- クロスコンパイル対応 (x86_64/aarch64 × Linux/macOS/Windows)
- README.md・使い方ガイドの整備
