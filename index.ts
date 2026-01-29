import { get, create } from "./agents.ts";

const agents = await get();
console.log(JSON.stringify(agents, null, 2));

