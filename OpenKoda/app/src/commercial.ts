import { request } from './api';
export type AccountUser = {id:string;email:string};
export type Reasoning = 'low'|'medium'|'high';
export type CommercialModel = {id:string;reasoningLevels:Reasoning[];contextLength:number;maxOutputTokens:number;image:boolean};
export type Skill = {id:string;name:string;description:string;revision:string;invocation:string};
export type AccountView = {
  plans?:{id:string;priceBrlCents:number;monthlyLimitMicros:number}[];
  plan:null|{id:string;priceBrlCents:number;monthlyLimitMicros:number};
  subscription:null|{status:string;periodStart?:string;periodEnd?:string;mode?:string;cancelAtPeriodEnd?:boolean};
  usage?:{monthLimitMicros?:number;windowLimitMicros?:number;monthUsedMicros?:number;windowUsedMicros?:number;
    monthlyLimitMicros?:number;monthlyUsedMicros?:number;windowEnd?:string;periodEnd?:string};
  controls?:{inferenceEnabled?:boolean;paymentsEnabled?:boolean;mockCheckout?:boolean};
  consent?:{termsVersion?:string;privacyVersion?:string;requiredTermsVersion?:string;requiredPrivacyVersion?:string;marketing?:boolean}|null;
};
export const accountChanged='openkoda:account-change';
export const usdMicros=(amount:number)=>new Intl.NumberFormat('pt-BR',{style:'currency',currency:'USD',maximumFractionDigits:6}).format(amount/1e6);
export async function getAccount(){
  const {user}=await request<{user:AccountUser|null}>('account.session');
  if(!user)return {user:null,account:null};
  const account=await request<AccountView>('account.overview');
  return {user,account};
}
export async function site(page:'support'|'account'|'plans'|'privacy'|'terms'|'refund'|'signup'|'signin'){
  await request('account.site',{page});
}
