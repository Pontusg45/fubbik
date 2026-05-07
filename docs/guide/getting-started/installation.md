---
tags:
  - guide
  - onboarding
  - installation
description: How to clone, install, and set up fubbik
---

# Installation

Clone the repo and install dependencies:

```bash
git clone <repo-url>
cd fubbik
pnpm install
```

Copy the environment file and configure:

```bash
cp .env.example .env
```

Push the database schema and seed sample data:

```bash
pnpm db:push
pnpm seed
```

Start the development server:

```bash
pnpm dev
```

This starts both the API server (port 3000) and web app (port 3001).
