#!/bin/bash

# Cleanup script for old tools after successful uv migration
# Run this script only after verifying that uv migration works correctly

set -e

echo "🧹 Cleaning up old tool configurations..."

# Files to remove
FILES_TO_REMOVE=(
    "poetry.lock.backup"
    ".python-version.backup"
    "requirements_dev.txt"
    "requirements_test.txt"
)

# Directories to remove (if they exist)
DIRS_TO_REMOVE=(
    ".tox"
    "dql.egg-info"
    "build"
    "dist"
)

echo "📁 Removing old configuration files..."
for file in "${FILES_TO_REMOVE[@]}"; do
    if [ -f "$file" ]; then
        rm "$file"
        echo "✅ Removed $file"
    else
        echo "ℹ️  $file not found, skipping"
    fi
done

echo "📂 Removing old build directories..."
for dir in "${DIRS_TO_REMOVE[@]}"; do
    if [ -d "$dir" ]; then
        rm -rf "$dir"
        echo "✅ Removed $dir"
    else
        echo "ℹ️  $dir not found, skipping"
    fi
done

echo ""
echo "✅ Cleanup completed!"
echo ""
echo "📋 Remaining files:"
echo "- pyproject.toml (updated for uv)"
echo "- uv.lock (new lock file)"
echo "- .venv/ (uv virtual environment)"
echo "- .envrc (updated for uv)"
echo "- tox.ini (updated for uv)"
echo ""
echo "🎉 Your project is now fully migrated to uv!"
