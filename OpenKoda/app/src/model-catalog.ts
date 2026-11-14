export type ModelCatalogEntry = {
  id:string; name:string; chat:boolean; analysis:boolean; selectable:boolean;
  availability:'eligible'|'policy'|'unsupported'; reason?:string|null;
};
