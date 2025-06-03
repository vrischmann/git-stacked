 # git stacked

This is not a tool to for stacked PRs, at least not a complete tool.

Currently it's only a viewer for stacked branches.

An example will make things clearer, let's I have the following branches:
* `main` is my main branch
* `feature-A` is for feature A
* `feature-B` is for feature B which depends on feature A
* `feature-C` is for feature C which depends on feature B
* `feature-D` is for feature D which depends on feature A
* `feature-E` is for feature E which depends on feature D

I personally can managed the rebasing manually just fine without a tool (at least for now) however keeping track of the trees so that I know what I need to rebase is harder, and annoying. `git stacked` helps me visualize my trees and make this easier.

With this example the output looks like this:
```
master
└── feature-A
    ├── feature-B
    │   └── feature-C
    └── feature-D
        └── feature-E
```

If you commit on `master` next the output looks like this:
```
(detached) feature-A
├── feature-B
│   └── feature-C
└── feature-D
    └── feature-E
master
```

which tells you that `feature-A` and below are now detached from `master`.

So, is this actually useful ? I just made this tool (well Gemini actually), only time will tell if it helps me.
