---
tags:
  - guide
  - cli
  - plans
description: CLI commands for managing plans, steps, and requirements
---

# Plan and Requirement Commands

## Plans

```bash
# Create a plan
fubbik plan create "Implement auth" --template feature-dev

# Import a plan from markdown
fubbik plan import plan.md

# List plans
fubbik plan list

# Show plan details
fubbik plan show <id>

# Update plan status
fubbik plan status <id> active

# Mark a step as done
fubbik plan step-done <plan-id> <step-number>

# Add a new step
fubbik plan add-step <plan-id> "Deploy to staging"

# Link a requirement
fubbik plan link-requirement <plan-id> <requirement-id>
```

## Requirements

```bash
# Create a requirement with BDD steps
fubbik requirements add "User login" \
  --step "given: a user exists" \
  --step "when: they enter credentials" \
  --step "then: they are logged in"

# Export requirements
fubbik requirements export --format gherkin   # Cucumber .feature files
fubbik requirements export --format vitest    # TypeScript test scaffolds
fubbik requirements export --format markdown  # Checklist format
```
