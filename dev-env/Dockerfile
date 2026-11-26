# syntax=docker/dockerfile:1
# A Docker sandbox template for maintaining this repository: Docker's Claude Code template plus
# what `just verify` and the agents need (Rust, just, uv, Chrome, Java, the Cursor CLI, ...).
# The repository is mounted at run time, not copied in. See MAINTAINER.md, "Dev environment".
#
#   docker build -t dql-dev dev-env
#   docker image save dql-dev -o dql-dev.tar && sbx template load dql-dev.tar
#   sbx run --template dql-dev claude
#
# The base is Ubuntu with a non-root `agent` user (UID 1000, passwordless sudo) and Node.js,
# Python, uv, Java, and Claude Code installed.
ARG BASE_IMAGE=docker/sandbox-templates:claude-code
FROM ${BASE_IMAGE}

USER root
COPY init.sh /opt/dql-dev/init.sh
RUN chmod 0755 /opt/dql-dev/init.sh

# Run as the agent user: rustup, uv, just, and the agent CLIs install into /home/agent, and the
# apt steps go through sudo. Running as root would put them under /root, where the agent can't use them.
USER agent
ENV PATH=/home/agent/.cargo/bin:/home/agent/.local/bin:${PATH}
# Steps for the image. The repository isn't in the image, so the repo step runs later, in the
# sandbox: dev-env/init.sh repo. Example: --build-arg INIT_ARGS="--no-repo --skip browser"
ARG INIT_ARGS=--no-repo
RUN /opt/dql-dev/init.sh ${INIT_ARGS}

# The sandbox starts agents through non-interactive bash, which reads BASH_ENV
# (/etc/sandbox-persistent.sh) instead of a profile. Keep the home-directory tools on PATH there too.
RUN echo 'export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"' | sudo tee -a /etc/sandbox-persistent.sh >/dev/null
