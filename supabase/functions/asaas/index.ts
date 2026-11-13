import { createBillingHandler } from './handler.mjs';

Deno.serve(createBillingHandler({env:(name:string)=>Deno.env.get(name)}));
