#!/bin/bash

# Claude Code Dynamic Statusline Script  
# Shows model, working directory, git branch, and system info
# Fully dynamic - adapts to any Claude model automatically

# Debug mode: uncomment to see raw JSON input
# cat >&2

# Read JSON input from Claude Code
input=$(cat)

# Dynamic model name extraction with multiple fallback strategies
extract_model_name() {
    local json_input="$1"
    local model_name=""
    
    if command -v jq > /dev/null 2>&1; then
        # Try multiple JSON fields in order of preference
        model_name=$(echo "$json_input" | jq -r '
            .model.display_name // 
            .model.name // 
            .model.family // 
            .model.id // 
            .model.type // 
            (.model | keys[0] as $k | .[$k]) //
            "AI Model"
        ' 2>/dev/null)
    else
        # Robust fallback parsing without jq - try multiple patterns
        model_name=$(echo "$json_input" | grep -o '"model"[^}]*"display_name"[^"]*"[^"]*"' | sed 's/.*"display_name":"//;s/".*//' 2>/dev/null)
        
        if [[ -z "$model_name" ]]; then
            model_name=$(echo "$json_input" | grep -o '"model"[^}]*"name"[^"]*"[^"]*"' | sed 's/.*"name":"//;s/".*//' 2>/dev/null)
        fi
        
        if [[ -z "$model_name" ]]; then
            model_name=$(echo "$json_input" | grep -o '"model"[^}]*"family"[^"]*"[^"]*"' | sed 's/.*"family":"//;s/".*//' 2>/dev/null)
        fi
        
        if [[ -z "$model_name" ]]; then
            # Extract any string value from model object as last resort
            model_name=$(echo "$json_input" | grep -o '"model"[^}]*"[^"]*"[^"]*"' | grep -o '"[^"]*"$' | sed 's/"//g' | head -1 2>/dev/null)
        fi
        
        # Generic fallback if nothing found
        if [[ -z "$model_name" ]]; then
            model_name="AI Model"
        fi
    fi
    
    echo "$model_name"
}

# Extract information from JSON
if command -v jq > /dev/null 2>&1; then
    MODEL=$(extract_model_name "$input")
    DIR=$(echo "$input" | jq -r '.workspace.current_dir // "~"' 2>/dev/null)
    SESSION_ID=$(echo "$input" | jq -r '.session.id // ""' 2>/dev/null)
else
    MODEL=$(extract_model_name "$input")
    DIR=$(echo "$input" | grep -o '"workspace"[^}]*"current_dir"[^"]*"[^"]*"' | sed 's/.*"current_dir":"//;s/".*//' 2>/dev/null || echo "~")
fi

# Handle empty or invalid model name
if [[ -z "$MODEL" || "$MODEL" == "null" ]]; then
    MODEL="AI Model"
fi

# Get directory name (last part of path)
DIR_NAME=${DIR##*/}
if [[ -z "$DIR_NAME" || "$DIR_NAME" == "/" ]]; then
    DIR_NAME="root"
fi

# Get git information if in git repository
GIT_INFO=""
if [[ -d "$DIR/.git" ]] || git -C "$DIR" rev-parse --git-dir > /dev/null 2>&1; then
    # Get repository name (bright yellow for distinction)
    REPO_ROOT=$(git -C "$DIR" rev-parse --show-toplevel 2>/dev/null)
    if [[ -n "$REPO_ROOT" ]]; then
        REPO_NAME=$(basename "$REPO_ROOT")
        GIT_REPO=" \033[93m⭐$REPO_NAME\033[0m"  # Bright yellow
    fi
    
    # Get current branch (bright colors for better visibility)
    BRANCH=$(git -C "$DIR" branch --show-current 2>/dev/null)
    if [[ -n "$BRANCH" ]]; then
        # Check for uncommitted changes
        if ! git -C "$DIR" diff-index --quiet HEAD -- 2>/dev/null; then
            GIT_BRANCH=" \033[91m📍$BRANCH*\033[0m"  # Bright red with asterisk for changes
        else
            GIT_BRANCH=" \033[92m📍$BRANCH\033[0m"   # Bright green for clean
        fi
    fi
    
    # Get current commit ID (bright magenta for uniqueness)
    COMMIT_ID=$(git -C "$DIR" rev-parse --short HEAD 2>/dev/null)
    if [[ -n "$COMMIT_ID" ]]; then
        GIT_COMMIT=" \033[95m🏷️$COMMIT_ID\033[0m"  # Bright magenta
    fi
    
    # Combine git information
    GIT_INFO="$GIT_REPO$GIT_BRANCH$GIT_COMMIT"
fi

# Truncate session ID to first 8 characters for display (bright cyan for distinction)
if [[ -n "$SESSION_ID" ]]; then
    SHORT_SESSION="${SESSION_ID:0:8}"
    SESSION_INFO=" \033[96m🔗$SHORT_SESSION\033[0m"  # Bright cyan
fi

# Smart model display formatting with enhanced colors
format_model_name() {
    local model="$1"
    local formatted_model=""
    local color_code=""
    
    # Detect model family and apply bold colors for better visibility
    case "$model" in
        *"Sonnet"*|*"sonnet"*)
            color_code="\033[1;35m"  # Bold Purple for Sonnet (premium model)
            # Extract version if present (e.g., "Sonnet 4" from "claude-sonnet-4-20250514")
            if [[ "$model" =~ [Ss]onnet.?([0-9]+(\.[0-9]+)?) ]]; then
                formatted_model="Sonnet ${BASH_REMATCH[1]}"
            else
                formatted_model="Sonnet"
            fi
            ;;
        *"Haiku"*|*"haiku"*)
            color_code="\033[1;32m"  # Bold Green for Haiku (fast model)
            if [[ "$model" =~ [Hh]aiku.?([0-9]+(\.[0-9]+)?) ]]; then
                formatted_model="Haiku ${BASH_REMATCH[1]}"
            else
                formatted_model="Haiku"
            fi
            ;;
        *"Opus"*|*"opus"*)
            color_code="\033[1;31m"  # Bold Red for Opus (powerful model)
            if [[ "$model" =~ [Oo]pus.?([0-9]+(\.[0-9]+)?) ]]; then
                formatted_model="Opus ${BASH_REMATCH[1]}"
            else
                formatted_model="Opus"
            fi
            ;;
        *"Claude"*|*"claude"*)
            color_code="\033[1;36m"  # Bold Cyan for generic Claude
            # Try to extract version number
            if [[ "$model" =~ [Cc]laude.?([0-9]+(\.[0-9]+)?) ]]; then
                formatted_model="Claude ${BASH_REMATCH[1]}"
            else
                formatted_model="Claude"
            fi
            ;;
        *)
            color_code="\033[1;33m"  # Bold Yellow for unknown/other models
            # Truncate long model names but preserve important info
            if [[ ${#model} -gt 20 ]]; then
                formatted_model="${model:0:17}..."
            else
                formatted_model="$model"
            fi
            ;;
    esac
    
    echo -e "${color_code}${formatted_model}\033[0m"
}

# Format the model name intelligently
FORMATTED_MODEL=$(format_model_name "$MODEL")

# Build enhanced statusline with distinctly colored elements
# Format: [Model] 📁 directory ⭐repo 📍branch 🏷️commit 🔗session
echo -e "[${FORMATTED_MODEL}] 📁 \033[94m$DIR_NAME\033[0m$GIT_INFO$SESSION_INFO"