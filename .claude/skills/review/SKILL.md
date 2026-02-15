---
name: review
description: PR前の包括レビュー。diff分析→チームレビュー→修正サイクル→再レビューを実行。
allowed-tools: Read, Grep, Glob, Bash(git *), Task, TeamCreate, TeamDelete, TaskCreate, TaskUpdate, TaskList, TaskGet, SendMessage, AskUserQuestion
---

# 包括レビュースキル

全モジュールを並列レビューし、修正→再レビューのサイクルを回す。

## Step 1: スコープ判定

`git diff main...HEAD --name-only` で変更ファイルを検出し、[execute-review/SKILL.md](../execute-review/SKILL.md) のパスマッピングで対象モジュールを特定する。

対象0件 → 「差分なし」報告で終了。

## Step 2: チームレビュー

```
TeamCreate("cb-review")
```

対象モジュールごとに **並列で** サブエージェントを起動:

```
Task({
  subagent_type: "general-purpose", team_name: "cb-review",
  name: "reviewer-{module}", model: "sonnet",
  prompt: ".claude/skills/execute-review/SKILL.md を読み、{module} モジュールのレビューを実行してください。
           チームエージェントなので結果は SendMessage でリーダーに報告してください。"
})
```

## Step 3: 結果収集 + エージェント解放

全エージェントからの報告を待つ。全報告を受領したら、各レビューエージェントに `SendMessage(type: "shutdown_request")` を送信して解放する。

## Step 4: 修正サイクル

全モジュールの結果をまとめて [request-fix/SKILL.md](../request-fix/SKILL.md) の手順で処理する。

全指摘が「なし」の場合は Step 7 へ。

## Step 5: 再スコープ判定

修正されたファイルのパスから直接変更されたモジュールを特定する。さらに、[execute-review/SKILL.md](../execute-review/SKILL.md) のパスマッピングの **依存先** 列を参照し、依存先が修正されたモジュールも再レビュー対象に加える。

例: core のコードが修正された場合 → core に依存する wasm, docs も再レビュー対象

## Step 6: 再レビュー（1回のみ）

Step 5 で特定した対象モジュールについて、Step 2-3 を再実行する（既存チーム使い回し）。再レビューエージェントも報告受領後にシャットダウンする。

## Step 7: 最終報告 + チーム解散

```markdown
# レビュー結果サマリー

## 対象モジュール
- {一覧}

## 各モジュールの結果
{レビュー結果}

## 修正サイクル結果
{request-fix の結果}

## 残タスク
- {未解決・別PR先送り、なければ「なし」}
```

```
TeamDelete
```
