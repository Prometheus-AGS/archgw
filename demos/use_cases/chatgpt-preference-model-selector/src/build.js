const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

console.log('Starting the custom build process for the Chrome Extension...');

// Define paths
const contentScriptSource = path.join(__dirname, '..', 'src', 'scripts', 'content.js');
const buildDir = path.join(__dirname, '..', 'build');
const contentScriptDestDir = path.join(buildDir, 'static', 'js');

// Step 1: Run the standard React build script
try {
  console.log('Running react-scripts build...');
  execSync('react-scripts build', { stdio: 'inherit' });
  console.log('React build completed successfully.');
} catch (error) {
  console.error('React build failed. Please check the errors above.');
  process.exit(1);
}

// Step 2: Copy the content script to the build directory
try {
  // Ensure the destination directory exists (it should after the build)
  if (fs.existsSync(contentScriptDestDir)) {
    fs.copyFileSync(contentScriptSource, path.join(contentScriptDestDir, 'content.js'));
    console.log(`Successfully copied content.js to ${contentScriptDestDir}`);
  } else {
    throw new Error(`Destination directory not found: ${contentScriptDestDir}. The build might have failed.`);
  }
} catch (error) {
  console.error('Failed to copy content script:', error);
  process.exit(1);
}

console.log('Extension build process finished successfully!');
