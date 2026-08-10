// Test if the content already contains a title with '# ...', or if we need to
// take it from the frontmatter
export const testForTitle = (content: string): boolean => {
  const lines = content.split(/\r?\n/);
  let isFrontMatterLine = false;
  for (const line of lines) {
    const trimmedLine = line.trim();

    // Toggle isFrontMatterLine when encountering '---'
    if (trimmedLine.startsWith("---")) {
      isFrontMatterLine = !isFrontMatterLine;
      continue;
    }

    // Skip the line if it's within the frontmatter
    if (isFrontMatterLine) {
      continue;
    }

    // Skip empty lines
    if (trimmedLine.length === 0) {
      continue;
    }

    // Check if the line starts with a title using '# '
    if (trimmedLine.startsWith("# ")) {
      return true;
    } else {
      // If the line doesn't start with a title and it's not within the frontmatter,
      // return false, as it means the title is not present after the frontmatter
      return false;
    }
  }
  // If the loop completes without finding a title, return false
  return false;
};

export default testForTitle;
