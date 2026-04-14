# Automatic Reindexing (Watch Mode)

AgentMem can keep your project index fresh automatically while you work.

Instead of manually running:

```bash
agentmem reindex
```

# terminal 1
``` bash
agentmem watch
```

# terminal 2
``` bash
npm run dev
```

## example

src/auth/login.ts changed
-> reindex changed file
-> update local memory
-> search results stay fresh

## ignored folders:

.git
node_modules
target
dist
build
.next
coverage
.agentmem

# stop watchmode

CTRL + C

# if manual indexing is preferred use:

``` bash
agentmem reindex
````




