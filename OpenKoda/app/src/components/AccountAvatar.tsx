import { useState, type ReactNode } from 'react';

export function AccountAvatar({url,className='',fallback}:{url?:string;className?:string;fallback:ReactNode}){
  const [failed,setFailed]=useState<string>();
  return url&&failed!==url?<img className={`account-avatar-image ${className}`} src={url} alt="" referrerPolicy="no-referrer" onError={()=>setFailed(url)}/>:<>{fallback}</>;
}
