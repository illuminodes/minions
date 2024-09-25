#!/bin/bash

# Function to display error message and exit
error_exit() {
    echo "$1" 1>&2
    exit 1
}

# Hardcoded username and server
username="YOUR_SERVER_NAME"
hostname="YOUR_SERVER_IP"
project_name="minions"

# Run trunk build --release
trunk build --release --features test|| error_exit "trunk build --release failed."

# Check if 'dist' folder exists
if [ ! -d "dist" ]; then
    error_exit "'dist' folder not found!"
fi

# Create a folder with the project name and copy 'dist' folder into it
mkdir "$project_name" || error_exit "Failed to create folder with project name."
# Copy the contents of 'dist' folder into the folder with the project name
cp -r dist/* "$project_name" || error_exit "Failed to copy 'dist' folder contents."

# SCP the folder to the server
scp -r "$project_name" "$username@$hostname:~/" || error_exit "SCP failed."

# Clean up
rm -r "$project_name"
rm -r dist

# run remote script 
ssh "$username@$hostname" "bash -s" < remote_deploy.sh || error_exit "Remote script failed."

echo "Deployment successful and folders cleaned up."

