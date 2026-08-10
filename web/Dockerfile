FROM node AS frontend
WORKDIR /app
COPY package*.json ./
RUN npm install
COPY . .
RUN npx prisma generate
RUN npx openapi --input https://stablehorde.net/api/swagger.json --output ./src/generated/stable-horde-api --name stableHordeClient
RUN npm run build
CMD ["npm", "run", "prod"]
