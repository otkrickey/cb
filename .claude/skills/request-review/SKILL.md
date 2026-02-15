---
name: request-review
description: 指定モジュールのレビューをサブエージェントで実行し、修正サイクルを回す。引数にモジュール名（core/wasm/server/web/docs）を指定。
allowed-tools: Read, Grep, Glob, Bash(git *), Task, AskUserQuestion
---

# モジュールレビューリクエスト

単一モジュールのレビューをサブエージェントで実行し、修正サイクルを回す。

## Step 1: レビュー実行

```
Task({
  subagent_type: "general-purpose", model: "sonnet",
  name: "reviewer-{module}", description: "{module} レビュー実行",
  prompt: ".claude/skills/execute-review/SKILL.md を読み、{module} モジュールのレビューを実行してください。"
})
```

## Step 2: 修正サイクル

レビュー結果を [request-fix/SKILL.md](../request-fix/SKILL.md) の手順で処理する。

## Step 3: 結果報告

```markdown
# {Module} レビュー結果

## レビュー結果
{サブエージェントのレビュー結果}

## 修正サイクル結果
{request-fix の結果}
```
