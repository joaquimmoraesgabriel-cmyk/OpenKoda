// Original bounded spring sampler. Parameters follow the requested Motion Studies
// references; no gallery autoplay, remote assets, or global animation loop.
export const springs = {
  soft: [260, 24], fade: [420, 32], pop: [520, 16],
  lead: [620, 36], trail: [190, 19], word: [340, 26],
  copyLabel: [260, 20], copyIcon: [420, 18], copyWidth: [300, 24],
} as const;

export function springSamples(stiffness: number, damping: number) {
  const values = [0];
  let position = 0, velocity = 0;
  for (let frame = 0; frame < 120; frame++) {
    for (let sub = 0; sub < 4; sub++) {
      velocity += (-stiffness * (position - 1) - damping * velocity) / 240;
      position += velocity / 240;
    }
    values.push(position);
    if (frame > 10 && Math.abs(position - 1) < .001 && Math.abs(velocity) < .001) break;
  }
  values[values.length - 1] = 1;
  return values;
}

export function reducedMotion() {
  return document.documentElement.dataset.motion === 'reduced'
    || window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

const running=new Set<Animation>();
let preference:MediaQueryList|undefined,observer:MutationObserver|undefined;
function stopReducedMotion(){if(reducedMotion())for(const animation of [...running])animation.cancel();}
// A single observer serves all short word/control animations. Changing the app
// appearance setting or the OS preference stops motion already in progress.
export function respectMotionPreference(animation:Animation){
  if(!running.size){
    preference=window.matchMedia('(prefers-reduced-motion: reduce)');
    preference.addEventListener('change',stopReducedMotion);
    observer=new MutationObserver(stopReducedMotion);
    observer.observe(document.documentElement,{attributes:true,attributeFilter:['data-motion']});
  }
  running.add(animation);
  const finished=()=>{
    running.delete(animation);
    animation.removeEventListener('finish',finished);animation.removeEventListener('cancel',finished);
    if(!running.size){observer?.disconnect();preference?.removeEventListener('change',stopReducedMotion);}
  };
  animation.addEventListener('finish',finished);animation.addEventListener('cancel',finished);
  return animation;
}

export function springAnimate(node: Element, frame: (value: number) => Keyframe,
  preset: readonly [number, number] = springs.fade) {
  if (reducedMotion() || !node.animate) return undefined;
  const samples = springSamples(...preset);
  return respectMotionPreference(node.animate(samples.map(frame), { duration: (samples.length - 1) * 1000 / 60, easing: 'linear' }));
}
