import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { BoxedImage } from "./BoxedImage";

jest.mock("uuid", () => ({ v4: () => "caption-id" }));
jest.mock("react-photoswipe-gallery", () => ({
  Gallery: () => null,
  Item: () => null,
}));
jest.mock("./ImageCaption/ImageCaption", () => ({
  ImageCaption: () => null,
}));

describe("BoxedImage", () => {
  test("renders the responsive placeholder before an image is available", () => {
    const html = renderToStaticMarkup(
      <BoxedImage
        alt="Generated illustration"
        className="article-image"
        prompt="An image still being generated"
      />,
    );

    expect(html).toContain('src="/placeholder.jpeg"');
    expect(html).toContain("article-image");
    expect(html).toContain('aria-busy="true"');
  });
});
