// Independent JS fixtures based on the pinned upstream type inventory. Values
// are not produced by the Rust model, so missing fields cannot self-confirm.
export function corpus(snapshot) {
  let ordinal = 0;
  const base = (type, extra={}) => {
    const i=ordinal++;
    return {type,id:`${type}-${i}`,x:60+(i%4)*220,y:60+Math.floor(i/4)*180,width:120,height:80,
      angle:0,strokeColor:"#123456",backgroundColor:"transparent",fillStyle:"solid",strokeWidth:2,
      strokeStyle:"solid",roundness:null,roughness:0,opacity:100,seed:i+1,version:1,versionNonce:0,
      index:`a${i.toString(36)}`,isDeleted:false,groupIds:[],frameId:null,boundElements:[],updated:1,
      ...(snapshot?{created:null}:{}),link:null,locked:false,customData:{fixture:true},...extra};
  };
  const linear = {points:[[0,0],[100,50]],startBinding:null,endBinding:null,startArrowhead:null,endArrowhead:null,
    ...(snapshot?{}:{lastCommittedPoint:null})};
  const elements = [base("rectangle",{backgroundColor:"#aabbcc"}),base("diamond"),base("ellipse"),
    base("text",{fontSize:20,fontFamily:5,text:"Editable note",originalText:"Editable note",textAlign:"left",verticalAlign:"top",containerId:null,autoResize:true,lineHeight:1.25,height:25,...(snapshot?{baseFontSize:null,labelPosition:null}:{})}),
    base("line",{...linear,...(snapshot?{polygon:false}:{})}),base("arrow",{...linear,endArrowhead:"triangle",elbowed:false}),
    base("freedraw",{points:[[0,0],[30,20],[80,0]],pressures:[0.2,0.8,0.4],simulatePressure:false,...(snapshot?{strokeOptions:{variability:"variable",streamline:0.5}}:{lastCommittedPoint:null})}),
    base("image",{fileId:"pixel",status:"saved",scale:[-1,1],crop:null}),
    base("frame",{name:"Frame"}),base("magicframe",{name:"Magic"}),
    base("iframe",{customData:{generationData:{status:"done",html:"<p>Local fixture</p>"},extra:true}}),
    base("embeddable",{link:"https://example.test/document"})];
  if(snapshot) elements.push(base("stickynote",{baseHeight:80,backgroundColor:"#fff9db"}));
  return {type:"excalidraw",version:2,source:"independent-corpus",elements,
    appState:{viewBackgroundColor:"#ffffff",gridSize:20,gridStep:5,gridModeEnabled:false,...(snapshot?{lockedMultiSelections:{}}:{})},
    files:{pixel:{id:"pixel",mimeType:"image/png",created:1,lastRetrieved:2,version:1,
      dataURL:"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jRZkAAAAASUVORK5CYII=",extra:true}}};
}

export function relationshipCorpus(snapshot) {
  const doc=corpus(snapshot);
  const originals=doc.elements;
  const byType=type=>structuredClone(originals.find(e=>e.type===type));
  const box={...byType("rectangle"),id:"box",x:350,y:180,width:160,height:100,index:"a0",boundElements:[{id:"label",type:"text"},{id:"leader",type:"arrow"}]};
  const label={...byType("text"),id:"label",x:375,y:215,width:110,height:25,index:"a1",containerId:"box",text:"Bound label",originalText:"Bound label",textAlign:"center",verticalAlign:"middle"};
  const binding=snapshot?{elementId:"box",fixedPoint:[1,0.5],mode:"orbit"}:{elementId:"box",focus:0,gap:1};
  const leader={...byType("arrow"),id:"leader",x:700,y:230,width:189,height:0,index:"a2",points:[[0,0],[-189,0]],endBinding:binding};
  const elbowBinding=snapshot?{elementId:"target",fixedPoint:[0,0.5],mode:"orbit"}:{elementId:"target",focus:0,gap:1,fixedPoint:[0,0.5]};
  const target={...byType("ellipse"),id:"target",x:700,y:400,width:100,height:80,index:"a3",boundElements:[{id:"elbow",type:"arrow"}]};
  const elbow={...byType("arrow"),id:"elbow",index:"a4",x:500,y:360,width:199,height:80,points:[[0,0],[100,0],[100,80],[199,80]],endBinding:elbowBinding,elbowed:true,fixedSegments:[{start:[100,0],end:[100,80],index:2}],startIsSpecial:false,endIsSpecial:false};
  doc.elements=[box,label,leader,target,elbow];
  if(snapshot) {
    const sticky={...byType("stickynote"),id:"sticky",index:"a5",x:350,y:440,width:180,height:120,baseHeight:120,boundElements:[{id:"sticky-label",type:"text"}]};
    const note={...label,id:"sticky-label",index:"a6",x:375,y:470,width:130,text:"Sticky label",originalText:"Sticky label",containerId:"sticky",baseFontSize:20};
    doc.elements.push(sticky,note);
  }
  doc.files={};
  return doc;
}
