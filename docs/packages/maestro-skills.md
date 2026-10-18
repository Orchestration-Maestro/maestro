# Development skills

`packages/maestro-skills/` is a content-only package of development skills and
shared reference documents. It uses conventional package discovery and the
existing [skill loader](../resources/skills.md); it contains no executable code.
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

This selects package resources through the resolver. The embedding caller then
passes enabled skill paths to the skill loader. It does not require a build.
Once base-package installation is delivered, the pack will be installed by
default; local-path loading does not depend on that installation.

## Filter or override

Use an object-form package entry to exclude a delivered skill:

```json
{
  "packages": [
    {
      "source": "/path/to/maestro/packages/maestro-skills",
      "skills": [
        "!skills/maestro-retro"
      ]
    }
  ]
}
```

When `maestro-teach` is present in a later delivery, the same filter can use
`"skills": ["!skills/maestro-teach"]`. That example does not imply it is present
in this delivery.

A user or project skill with the same name overrides the packaged skill under
[configured resource precedence](../packages.md#configured-resource-resolution).
Keep the replacement's `name` equal to its directory name.

## Invocation

Model-invoked skills appear in the model's skill list. Type-only skills set
`disable-model-invocation: true` and stay out of that list; the router is type-only.
Their content can be loaded today. Typed `/skill:maestro-ask` expansion is a
separate delivery and is not implemented by this package.
