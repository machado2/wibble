/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  outputFileTracing: false,
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
