/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  outputFileTracing: false,
  experimental: {
    externalDir: true,
  },
  async rewrites() {
    return [
      {
        source: "/image/:id",
        destination: "/api/image/:id",
      },
    ];
  },
};

module.exports = nextConfig;
