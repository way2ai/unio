# 技能

技能让仓库或用户用 Markdown 定义可复用工作流。Unio 会从工作区和用户目录发现技能。

## 发现路径

```text
{workspace}/.unio/skills/
~/.unio/skills/
```

## 创建工作区技能

```powershell
New-Item -ItemType Directory -Force .unio\skills\repo-helper
Set-Content .unio\skills\repo-helper\SKILL.md "# Repo helper`nUse for repository analysis."
```

列出已发现技能：

```powershell
cargo run -p unio -- skills
```

## 调用技能

```powershell
cargo run -p unio -- tool skill-tool --approval full-trust --args name=repo-helper,request=inspect-modules
```

## 技能合同

- 技能位于包含 `SKILL.md` 的目录中。
- 第一段应说明技能适用场景。
- 额外脚本或素材应留在技能目录内。
- 技能执行使用与其他操作相同的工具和审批路径。
