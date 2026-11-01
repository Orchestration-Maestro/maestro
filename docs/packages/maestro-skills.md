# Development skills

`packages/maestro-skills/` is a content-only package of development skills and
shared reference documents. It uses conventional package discovery and the
existing [skill loader](../resources/skills.md). For creating your own skill or
choosing discovery paths, see [create and use skills](../skills.md).
It adds no runtime component; loading
does not execute the included Bash wizard-authoring template.
The `maestro-ask` router is the skill list and explains when to use each skill.

## Load from a local checkout

Add the absolute package path to `packages` in your user or project settings:

```json
{
  "packages": [
    "/path/to/maestro/packages/maestro-skills"
  ]
}
```

The application resolves the package and loads its enabled skills when it starts
or reloads resources. The content-only pack does not require a build.
[Default base-package installation](https://github.com/Orchestration-Maestro/maestro/issues/1578)
is not delivered; local-path loading does not depend on it.

## Filter or override

Use an object-form package entry to exclude a delivered skill:

```json
{
  "packages": [
    {
      "source": "/path/to/maestro/packages/maestro-skills",
      "skills": [
        "!skills/maestro-teach"
      ]
    }
  ]
}
```

A user or project skill with the same name overrides the packaged skill under
[configured resource precedence](../packages.md#configured-resource-resolution).
Keep the replacement's `name` equal to its directory name.

## Invocation

Model-invoked skills appear in the model's skill list. Type-only skills set
`disable-model-invocation: true` and stay out of that list; the router is type-only.
Type `/skill:maestro-ask` in Maestro to load the router. The application expands
skill commands; the package supplies their instructions. Text after the command
is passed after the skill body, for example:

```text
/skill:maestro-ask Help me choose a development workflow for this change.
```

See [skill invocation](../skills.md#invoke-a-skill) for command completion,
arguments and the distinction between explicit invocation and model selection.
