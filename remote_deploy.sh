#!/bin/bash

# Define variables
NEW_FOLDER="YOUR_SERVER_DIR" # Path to the new folder containing the binary and public folder
OLD_FOLDER="YOUR_SERVER_WWW_DIR" # Path to the old folder containing the binary and public folder

# Remove the old folder contents
echo "Removing old public folder..."
rm -rf "$OLD_FOLDER"/*

# Check if the old folder was removed successfully
if [ "$(ls -A "$OLD_FOLDER")" ]; then
    echo "Failed to remove the old public folder. Exiting."
    exit 1
fi

# Copy the new public folder to the old location
echo "Copying new folder content to the old location..."
cp -r "$NEW_FOLDER"/* "$OLD_FOLDER"/

# Check if the new public folder was copied successfully
if [ ! -d "$OLD_FOLDER" ]; then
    echo "Failed to copy the new public folder. Exiting."
    exit 1
fi

# clean up
rm -rf "$NEW_FOLDER"

echo "Deployment completed successfully."

