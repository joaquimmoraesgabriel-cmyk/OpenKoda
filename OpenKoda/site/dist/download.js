const trigger=document.querySelector('[data-installer]');
trigger?.addEventListener('click',()=>{
 const status=document.getElementById('download-status');
 status.textContent='Download solicitado. Confira a lista de downloads do navegador. Se a transferência não aparecer, use uma das alternativas abaixo.';
});
