# syntax=docker/dockerfile:1
#
# Postgres 18 + pgvector 0.8.2 + Apache AGE for fubbik Docker Compose.
#
# Multi-stage: compile AGE against dev headers, then copy only extension artifacts
# into a fresh pgvector layer so we never `apt remove` PostgreSQL packages.

ARG PGVECTOR_IMAGE=pgvector/pgvector:0.8.2-pg18

FROM ${PGVECTOR_IMAGE} AS age-builder

ARG AGE_REF=refs/tags/PG18/v1.7.0-rc0

ADD https://github.com/apache/age.git#${AGE_REF} /tmp/age

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    apt-mark hold locales \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      flex \
      bison \
      perl \
      postgresql-server-dev-18 \
 && cd /tmp/age \
 && make OPTFLAGS="" \
 && make install \
 && apt-mark unhold locales \
 && rm -rf /tmp/age

FROM ${PGVECTOR_IMAGE}

COPY --from=age-builder /usr/lib/postgresql/18/lib/age.so /usr/lib/postgresql/18/lib/
COPY --from=age-builder /usr/share/postgresql/18/extension/ /usr/share/postgresql/18/extension/
