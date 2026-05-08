# Skills

Skills let a repository or user define reusable workflows in Markdown. Unio
discovers skills from workspace and user directories.

## Discovery Paths

```text
{workspace}/.unio/skills/
~/.unio/skills/
```

## Create A Workspace Skill

```powershell
New-Item -ItemType Directory -Force .unio\skills\repo-helper
Set-Content .unio\skills\repo-helper\SKILL.md "# Repo helper`nUse for repository analysis."
```

List discovered skills:

```powershell
cargo run -p unio -- skills
```

## Invoke A Skill

```powershell
cargo run -p unio -- tool skill-tool --approval full-trust --args name=repo-helper,request=inspect-modules
```

## Skill Contract

- A skill lives in a directory with a `SKILL.md` file.
- The first paragraph should state when the skill applies.
- Extra scripts or assets should stay inside the skill directory.
- Skill execution uses the same tool and approval path as other operations.
