---
name: Feature Request (with Security Review)
about: Propose a new feature with mandatory security checklist
title: "[FEATURE] <title>"
labels: ["enhancement", "security-review-required"]
---

## Description
_Describe the feature briefly._

## Acceptance Criteria
- [ ] ...

## Security Checklist

**Does this feature accept user input?** (CLI args, HTTP bodies, config files)
- [ ] Yes / No
- If yes, describe validation strategy:

**Does this feature make network calls?**
- [ ] Yes / No
- If yes:
  - [ ] All URLs use HTTPS
  - [ ] TLS verification enabled
  - [ ] Timeout set on all requests

**Does this access files, environment, or spawn processes?**
- [ ] Yes / No
- If yes, describe sandboxing and validation:

**Could this enable SSRF, injection, privilege escalation, or DoS?**
- [ ] Yes / No
- If yes, threat model:

**Related Security Docs:**
- See [SECURITY.md](https://github.com/ehrenfest-quantum/quasi/blob/main/SECURITY.md) for baseline requirements
