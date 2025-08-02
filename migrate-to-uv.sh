#!/bin/bash

# Migration script: Poetry + pyenv + virtualenv → uv
# This script helps migrate from the current setup to uv

set -e

echo "🚀 Starting migration to uv..."

# Check if uv is installed
if ! command -v uv >/dev/null 2>&1; then
    echo "❌ uv is not installed. Installing uv..."
    curl -LsSf https://astral.sh/uv/install.sh | sh
    echo "✅ uv installed successfully"
    echo "Please restart your shell or run: source ~/.bashrc"
    exit 1
fi

echo "✅ uv is already installed"

# Backup current environment
echo "📦 Backing up current environment..."
if [ -f "poetry.lock" ]; then
    cp poetry.lock poetry.lock.backup
    echo "✅ Backed up poetry.lock"
fi

if [ -d ".venv" ]; then
    echo "⚠️  Found existing .venv directory"
    read -p "Do you want to remove the existing .venv? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf .venv
        echo "✅ Removed existing .venv"
    fi
fi

# Remove pyenv-specific files
echo "🧹 Cleaning up pyenv-specific files..."
if [ -f ".python-version" ]; then
    mv .python-version .python-version.backup
    echo "✅ Backed up .python-version"
fi

# Initialize uv
echo "🔧 Initializing uv..."
uv init --no-readme

# Install dependencies
echo "📥 Installing dependencies with uv..."
uv sync --dev

# Create new virtual environment
echo "🌍 Creating virtual environment..."
uv venv

echo "✅ Migration completed successfully!"
echo ""
echo "📋 Next steps:"
echo "1. Activate the virtual environment: source .venv/bin/activate"
echo "2. Test the installation: uv run pytest tests"
echo "3. Run linting: uv run mypy dql tests bin/install.py"
echo "4. Update your IDE to use the new .venv"
echo ""
echo "🔄 New commands:"
echo "- Install dependencies: uv sync"
echo "- Install dev dependencies: uv sync --dev"
echo "- Run commands: uv run <command>"
echo "- Add dependency: uv add <package>"
echo "- Add dev dependency: uv add --dev <package>"
echo ""
echo "📚 For more information, visit: https://docs.astral.sh/uv/"
