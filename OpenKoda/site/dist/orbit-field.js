// Original orbital square field: deterministic birth phases, live targets and
// bounded local input. No particles, samples or listeners are created per frame.
const TAU=Math.PI*2;
const clamp=(n,a,b)=>Math.max(a,Math.min(b,n));
const smooth=(a,b,n)=>{const t=clamp((n-a)/(b-a),0,1);return t*t*(3-2*t);};
const fraction=n=>n-Math.floor(n);
const seed=(i,s)=>fraction(Math.sin((i+1)*127.1+s*311.7)*43758.5453);

export function orbitVisibility(field,x,y){
 const nx=Math.abs(x-field.width*.5)/field.safeX,ny=Math.abs(y-field.height*.5)/field.safeY;
 // A calm rounded middle protects the complete title/subtitle/action group.
 return smooth(1,1.18,Math.pow(nx**4+ny**4,.25));
}

function position(field,i,time,initial=false){
 const travel=fraction(field.phase[i]+time*.021);
 const radius=1.32-travel*.65;
 const theta=field.crossPhase[i]+time*.19+travel*1.6;
 const breathing=1+.018*Math.sin(theta*3-time*.23);
 const x=field.width*.5+Math.cos(theta)*field.radiusX*radius*breathing;
 const y=field.height*.5+Math.sin(theta)*field.radiusY*radius*breathing;
 // The jump back to the outer orbit is completely invisible at both ends.
 const life=smooth(0,.13,travel)*(1-smooth(.76,1,travel));
 if(initial){field.x[i]=x;field.y[i]=y;}
 else{field.flowX[i]=x-field.x[i];field.flowY[i]=y-field.y[i];}
 field.coverage[i]=life*orbitVisibility(field,x,y);
}

export function createOrbitField(width,height,options,limits){
 const compact=width<700||height>width*1.25;
 const density=Number.isFinite(options.density)?clamp(options.density,.25,1):1;
 const count=Math.floor((compact||options.coarse?limits.compact:limits.desktop)*density);
 const field={scene:'aperture',width,height,count,compact,time:0,maxDisplacement:compact?44:68,
  radiusX:width*(compact?.76:.66),radiusY:height*(compact?.7:.69),
  safeX:width*(compact?.43:.38),safeY:Math.max(128,height*(compact?.27:.28)),
  pointerX:NaN,pointerY:NaN,pointerIdle:0,inputVX:0,inputVY:0};
 for(const key of ['x','y','size','alpha','tone','flowX','flowY','coverage','phase','crossPhase','heat','dx','dy','vx','vy','dissolve'])field[key]=new Float32Array(count);
 for(let i=0;i<count;i++){
  field.phase[i]=fraction(i*.61803398875);field.crossPhase[i]=seed(i,7)*TAU;
  field.size[i]=(compact?2.2:2.6)+seed(i,2)*(compact?3.4:4.7);
  field.alpha[i]=.2+seed(i,5)*.4;field.tone[i]=seed(i,9)>.91?.3:0;
  position(field,i,0,true);
 }
 return field;
}

export function evolveOrbitField(field,elapsed){
 field.time+=clamp(Number.isFinite(elapsed)?elapsed:0,0,.05);
 for(let i=0;i<field.count;i++){
  position(field,i,field.time);
 }
}

export function stepOrbitInput(field,pointer,dt){
 const active=!!pointer?.active&&Number.isFinite(pointer.x)&&Number.isFinite(pointer.y);
 let moving=false;
 if(active){
  const mx=Number.isFinite(field.pointerX)?pointer.x-field.pointerX:0,my=Number.isFinite(field.pointerY)?pointer.y-field.pointerY:0;
  if(Math.hypot(mx,my)>.25)field.pointerIdle=0;else field.pointerIdle+=dt;
  const ease=1-Math.exp(-11*dt);
  field.inputVX+=(clamp(mx/dt,-1000,1000)-field.inputVX)*ease;
  field.inputVY+=(clamp(my/dt,-1000,1000)-field.inputVY)*ease;
  field.pointerX=pointer.x;field.pointerY=pointer.y;
 }else{field.pointerIdle=0;field.pointerX=field.pointerY=NaN;field.inputVX*=Math.exp(-8*dt);field.inputVY*=Math.exp(-8*dt);}
 const presence=active?1-smooth(1.2,3.6,field.pointerIdle):0;
 const radius=clamp(Number.isFinite(pointer?.radius)?pointer.radius:Math.min(field.width,field.height)*.24,75,205);
 const omega=12,decay=Math.exp(-omega*dt),ease=1-Math.exp(-7*dt);
 for(let i=0;i<field.count;i++){
  let influence=0,targetX=0,targetY=0,dissolve=0;
  if(presence){
   const x=field.x[i]+field.flowX[i]-pointer.x,y=field.y[i]+field.flowY[i]-pointer.y;
   const distance=Math.hypot(x,y);
   if(distance<radius){
    influence=(1-smooth(0,radius,distance))*presence;
    const angle=distance>.001?Math.atan2(y,x):field.crossPhase[i],nx=Math.cos(angle),ny=Math.sin(angle);
    const reach=field.maxDisplacement*influence;
    // Inward gravity + clockwise tangential flow; drag impulse is strictly local.
    targetX=(-nx*.68-ny*.58)*reach+field.inputVX*.025*influence;
    targetY=(-ny*.68+nx*.58)*reach+field.inputVY*.025*influence;
    dissolve=influence*smooth(.12,1.1,field.pointerIdle)*.88;
   }
  }
  field.heat[i]+=(influence*.6-field.heat[i])*ease;
  field.dissolve[i]+=(dissolve-field.dissolve[i])*ease;
  if(!influence&&field.heat[i]<.001)field.heat[i]=0;
  if(!dissolve&&field.dissolve[i]<.001)field.dissolve[i]=0;
  const ox=field.dx[i]-targetX,oy=field.dy[i]-targetY;
  const sx=(field.vx[i]+omega*ox)*dt,sy=(field.vy[i]+omega*oy)*dt;
  let dx=targetX+(ox+sx)*decay,dy=targetY+(oy+sy)*decay;
  let vx=(field.vx[i]-omega*sx)*decay,vy=(field.vy[i]-omega*sy)*decay;
  const distance=Math.hypot(dx,dy);
  if(distance>field.maxDisplacement){const scale=field.maxDisplacement/distance;dx*=scale;dy*=scale;vx*=scale;vy*=scale;}
  if(!influence&&Math.abs(dx)+Math.abs(dy)+Math.abs(vx)+Math.abs(vy)<.015)dx=dy=vx=vy=0;
  field.dx[i]=dx;field.dy[i]=dy;field.vx[i]=vx;field.vy[i]=vy;
  if(Math.abs(dx)+Math.abs(dy)+field.dissolve[i]+field.heat[i]>.002)moving=true;
 }
 return moving;
}
