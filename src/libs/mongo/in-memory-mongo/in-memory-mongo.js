const { MongoMemoryReplSet } = require("mongodb-memory-server");

async function startInMemoryMongoDB() {
  const server = await MongoMemoryReplSet.create();

  const uri = server.getUri();

  console.log(uri);

  await new Promise(() => {});
}

startInMemoryMongoDB();
