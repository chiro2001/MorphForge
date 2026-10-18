// Read the original author-owned project without modifying it. Node needs its Three dependency.
import {pathToFileURL} from 'node:url';
import path from 'node:path';
import fs from 'node:fs';
const [source,output]=process.argv.slice(2);
if(!source||!output)throw new Error('Usage: node tools/export_legacy_world.mjs SOURCE_PROJECT OUTPUT.json');
const load=relative=>import(pathToFileURL(path.resolve(source,relative)).href);
const [{buildWorld},{createVoxelMaterials},{PALETTE}]=await Promise.all([load('src/world.js'),load('src/voxel.js'),load('src/palette.js')]);
const world=buildWorld(createVoxelMaterials());
const fields=['field','water'].map(name=>{const f=world[name];return{name,min:f.min,max:f.max,size:f.size,data:Buffer.from(f.data).toString('base64'),solidCount:f.solidCount}});
fs.writeFileSync(output,JSON.stringify({fields,palette:PALETTE,cellSize:world.modelScale,stats:world.stats,excludes:['miniGroup separate-scale animated clones','nonvoxel sky, lights and effects']}));
console.log(JSON.stringify({voxels:world.stats.voxels,output}));
