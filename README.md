# AgentMem

Local-first memory infrastructure for AI agents. AgentMem gives tools like Claude, Codex, Gemini, OpenClaw, Hermes, local coding agents and autonomous scripts a structured way to persist memory between runs.

Instead of losing context every session or scattering state across JSON files, temp folders, prompts, and shell history, AgentMem provides a clean shared memory layer.

---

# Why AgentMem Exists

Most AI tools today are stateless.

That means they repeatedly need to rediscover:

- what project this is
- where files live
- coding conventions
- current task status
- prior fixes
- known bugs
- open decisions
- team notes

That wastes:

- tokens
- time
- API cost
- focus
- context window space

AgentMem fixes that.

---

# Core Benefits

## Persistent Memory

Store useful project context between sessions.

## Local-First

Your memory stays on your machine.

## Fast CLI

Simple commands that agents and humans can both use.

## Structured Namespaces

Organize memory cleanly.

```text
agent/claude/current_task
project/demo/stack
repo/build_command
bugs/auth/login
```

######################################
##      Installation + Setup        ##
######################################

```bash
# install globally
cargo install agentmem

# verify install
agentmem --help

# go to your project
cd my-project

# initialize project memory and index codebase
agentmem init
agentmem index
# use re-index after changes to codebase
agentmem reindex

# store memory
agentmem set project/name "My Project"
agentmem set repo/stack "Next.js + TypeScript"
agentmem set project/current_goal "Ship billing v2"
agentmem set agent/claude/current_task "Fix auth bug"

# read memory
agentmem get project/name
agentmem get agent/claude/current_task

# list everything
agentmem list
```

######################################
##              Examples            ##
######################################

cd my-nextjs-app

agentmem init
agentmem set repo/stack "Next.js + Supabase"
agentmem set project/current_goal "Launch payments"
agentmem set agent/claude/current_task "Fix checkout bug"
agentmem get project/current_goal

