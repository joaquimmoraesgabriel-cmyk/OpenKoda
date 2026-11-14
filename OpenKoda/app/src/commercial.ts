import { request } from './api';
export type AccountUser = {id:string;email:string;displayName?:string;avatarUrl?:string};
export type AccountStatus = {state:'signedOut'|'authenticated'|'offline'|'expired';user:AccountUser|null;account:AccountView|null;message?:string};
export type AccountSimulation = {plan:'plus'|'pro'|'ultra';status:'active'|'canceled'|'expired';periodStart:string;periodEnd:string;mode:'internal'};
export type SimulationOutcome = 'activate'|'cancel'|'expire'|'fail';
export type SimulationResult = {status:'active'|'canceled'|'expired'|'failed';simulation:AccountSimulation|null;grantsProductionCredit:false;replay:boolean};
export type Reasoning = 'minimal'|'low'|'medium'|'high'|'xhigh'|'max';
export type CommercialModel = {id:string;reasoningLevels:Reasoning[];contextLength:number;maxOutputTokens:number;image:boolean;analysis?:boolean;chat?:boolean;name?:string;canonicalId?:string};
export type Skill = {id:string;name:string;description:string;revision:string;invocation:string};
export type AccountView = {
  plans?:{id:string;priceBrlCents:number;monthlyLimitMicros:number}[];
  plan:null|{id:string;priceBrlCents:number;monthlyLimitMicros:number};
  subscription:null|{status:string;periodStart?:string;periodEnd?:string;mode?:string;cancelAtPeriodEnd?:boolean};
  usage?:{monthLimitMicros?:number;windowLimitMicros?:number;monthUsedMicros?:number;windowUsedMicros?:number;
    monthlyLimitMicros?:number;monthlyUsedMicros?:number;windowEnd?:string;periodEnd?:string};
  controls?:{inferenceEnabled?:boolean;paymentsEnabled?:boolean;mockCheckout?:boolean;simulationEnabled?:boolean};
  simulation?:AccountSimulation|null;
  entitlements?:{reasoning:boolean;skills:boolean;hostedInference:boolean};
  consent?:{termsVersion?:string;privacyVersion?:string;requiredTermsVersion?:string;requiredPrivacyVersion?:string;marketing?:boolean}|null;
};
export const accountChanged='openkoda:account-change';
export const usdMicros=(amount:number)=>new Intl.NumberFormat('pt-BR',{style:'currency',currency:'USD',maximumFractionDigits:6}).format(amount/1e6);
export async function getAccount(){
  // One native snapshot binds the identity and entitlements while account
  // mutations are excluded. Offline restoration never returns old permissions.
  const {user,account}=await request<AccountStatus>('account.status');
  return {user,account};
}
export async function site(page:'support'|'account'|'plans'|'privacy'|'terms'|'refund'|'signup'|'signin'){
  await request('account.site',{page});
}
