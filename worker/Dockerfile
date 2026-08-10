FROM node
WORKDIR /app
ADD package*.json ./
RUN npm install
ADD . .
RUN npx prisma generate
CMD npm start
