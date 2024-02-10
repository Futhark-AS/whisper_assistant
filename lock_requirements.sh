#!/bin/bash

# Step 1: Freeze the environment
pip freeze > freeze.txt

# Step 2: Filter and prepare the locked requirements
while read requirement; do
    # Extract the package name, ignoring version specifiers
    pkg_name=$(echo $requirement | cut -d'=' -f1)
    # Look for the package in the freeze output and append it to the locked file
    grep "^$pkg_name==" freeze.txt >> requirements.txt
done < requirements.txt

# Cleanup
rm freeze.txt

echo "Locked versions are saved in requirements.txt"
