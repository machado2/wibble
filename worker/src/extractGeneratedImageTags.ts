import { remark } from "remark";
import { visit } from "unist-util-visit";
import remarkParse from "remark-parse";
import remarkMdx from "remark-mdx";

export interface GeneratedImageAttributes {
  prompt: string;
  alt: string;
  position: string;
}

export const extractGeneratedImageTags = (
  content: string
): GeneratedImageAttributes[] => {
  const extractedTags: GeneratedImageAttributes[] = [];

  // Parse the content using remark
  const parsedContent = remark().use(remarkMdx).use(remarkParse).parse(content);

  // Traverse the parsed content to find GeneratedImage tags
  visit(parsedContent, "mdxJsxFlowElement", (node: any) => {
    if (node.name === "GeneratedImage") {
      const attributes: GeneratedImageAttributes = {
        prompt: "",
        alt: "",
        position: "",
      };

      // Extract the attributes from the node
      node.attributes.forEach((attr: any) => {
        if (attr.name === "prompt") {
          attributes.prompt = attr.value;
        } else if (attr.name === "alt") {
          attributes.alt = attr.value;
        } else if (attr.name === "position") {
          attributes.position = attr.value;
        }
      });

      // Add the extracted attributes to the array
      extractedTags.push(attributes);
    }
  });

  return extractedTags;
};

export interface MarkdownImageAttributes {
  url: string;
  alt: string;
  title?: string;
}

export const extractMarkdownImages = (
  content: string
): MarkdownImageAttributes[] => {
  const extractedImages: MarkdownImageAttributes[] = [];

  // Parse the content using remark
  const parsedContent = remark().use(remarkParse).parse(content);

  // Traverse the parsed content to find Markdown images
  visit(parsedContent, "image", (node: any) => {
    const attributes: MarkdownImageAttributes = {
      url: node.url,
      alt: node.alt,
      title: node.title,
    };

    // Add the extracted attributes to the array
    extractedImages.push(attributes);
  });

  return extractedImages;
};
