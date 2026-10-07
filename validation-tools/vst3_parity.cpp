// A minimal offline VST3 host. Compares actual plugin float32 output bits.
// Build with the Steinberg VST3 pluginterfaces headers (bundled with JUCE here).
#include "pluginterfaces/base/ipluginbase.h"
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivsthostapplication.h"
#include "pluginterfaces/vst/ivstparameterchanges.h"
#include "pluginterfaces/vst/vstspeaker.h"
#include <dlfcn.h>
#include <algorithm>
#include <cmath>
#include <cstring>
#include <iomanip>
#include <iostream>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
using namespace Steinberg;
using namespace Steinberg::Vst;

static void check(tresult r, const char* what) {
    if (r != kResultOk) throw std::runtime_error(std::string(what) + " result=" + std::to_string(r));
}
static std::string text16(const TChar* p) {
    std::string s; while (*p) s.push_back(char(*p++)); return s;
}
// Objects below have host-owned lifetimes; the plugin can retain/release them.
#define UNKNOWN_METHODS(TYPE) \
 tresult PLUGIN_API queryInterface(const TUID id, void** out) override { \
   if (FUnknownPrivate::iidEqual(id, TYPE##_iid) || FUnknownPrivate::iidEqual(id, FUnknown_iid)) { \
     *out = static_cast<TYPE*>(this); addRef(); return kResultOk; } \
   *out = nullptr; return kNoInterface; } \
 uint32 PLUGIN_API addRef() override { return ++refs; } \
 uint32 PLUGIN_API release() override { return --refs; } \
 uint32 refs = 1;
struct Host : IHostApplication {
    UNKNOWN_METHODS(IHostApplication)
    tresult PLUGIN_API getName(String128 name) override { name[0]='P'; name[1]='a'; name[2]='r'; name[3]='i'; name[4]='t'; name[5]='y'; name[6]=0; return kResultOk; }
    tresult PLUGIN_API createInstance(TUID, TUID, void** out) override { *out=nullptr; return kNoInterface; }
};
struct Queue : IParamValueQueue {
    UNKNOWN_METHODS(IParamValueQueue)
    ParamID id; std::vector<std::pair<int32, double>> points;
    explicit Queue(ParamID p) : id(p) {}
    ParamID PLUGIN_API getParameterId() override { return id; }
    int32 PLUGIN_API getPointCount() override { return int32(points.size()); }
    tresult PLUGIN_API getPoint(int32 i, int32& offset, ParamValue& value) override {
        if (i<0 || i>=int32(points.size())) return kInvalidArgument;
        offset=points[i].first; value=points[i].second; return kResultOk;
    }
    tresult PLUGIN_API addPoint(int32 offset, ParamValue value, int32& index) override {
        index=int32(points.size()); points.emplace_back(offset,value); return kResultOk;
    }
};
struct Changes : IParameterChanges {
    UNKNOWN_METHODS(IParameterChanges)
    std::vector<std::unique_ptr<Queue>> queues;
    int32 PLUGIN_API getParameterCount() override { return int32(queues.size()); }
    IParamValueQueue* PLUGIN_API getParameterData(int32 i) override { return i>=0 && i<int32(queues.size()) ? queues[i].get() : nullptr; }
    IParamValueQueue* PLUGIN_API addParameterData(const ParamID& id, int32& index) override {
        for (size_t i=0;i<queues.size();++i) if (queues[i]->id==id) { index=int32(i); return queues[i].get(); }
        index=int32(queues.size()); queues.emplace_back(new Queue(id)); return queues.back().get();
    }
    void add(ParamID id,int32 offset,double value) { int32 q=0,p=0; addParameterData(id,q)->addPoint(offset,value,p); }
};
struct Plugin {
    void* module=nullptr; IPluginFactory* factory=nullptr;
    IComponent* component=nullptr; IAudioProcessor* processor=nullptr; IEditController* controller=nullptr;
    Host host; std::vector<ParameterInfo> params; bool active=false,processing=false;
    std::string name;
    explicit Plugin(const char* path) {
        module=dlopen(path,RTLD_NOW|RTLD_LOCAL); if (!module) throw std::runtime_error(dlerror());
        if (auto entry=reinterpret_cast<bool(*)(void*)>(dlsym(module,"bundleEntry"))) if (!entry(module)) throw std::runtime_error("bundleEntry");
        auto get=reinterpret_cast<IPluginFactory*(*)()>(dlsym(module,"GetPluginFactory"));
        if (!get) throw std::runtime_error("GetPluginFactory missing"); factory=get();
        PClassInfo info{}; bool found=false;
        for (int i=0;i<factory->countClasses();++i) { check(factory->getClassInfo(i,&info),"getClassInfo"); if (std::string(info.category)=="Audio Module Class") { found=true; break; } }
        if (!found) throw std::runtime_error("No audio plugin class");
        name=info.name;
        check(factory->createInstance(info.cid,IComponent_iid,reinterpret_cast<void**>(&component)),"createInstance");
        check(component->initialize(&host),"initialize");
        check(component->queryInterface(IAudioProcessor_iid,reinterpret_cast<void**>(&processor)),"processor interface");
        check(component->queryInterface(IEditController_iid,reinterpret_cast<void**>(&controller)),"controller interface");
        for (int i=0;i<controller->getParameterCount();++i) { ParameterInfo p{}; check(controller->getParameterInfo(i,p),"parameter info"); params.push_back(p); }
    }
    ~Plugin() {
        if (processing) processor->setProcessing(false);
        if (active) component->setActive(false);
        if (component) component->terminate();
        if (controller) controller->release(); if (processor) processor->release(); if (component) component->release();
        if (factory) factory->release();
        if (module) { if (auto leave=reinterpret_cast<bool(*)()>(dlsym(module,"bundleExit"))) leave(); dlclose(module); }
    }
    ParamID id(const std::string& name) const { for (const auto& p:params) if (text16(p.title)==name) return p.id; throw std::runtime_error("Missing parameter " + name); }
    void plain(const std::string& name,double value) { auto p=id(name); check(controller->setParamNormalized(p,controller->plainParamToNormalized(p,value)),"set parameter"); }
    void setup(double rate,int block,int channels,int profile) {
        if (profile!=0) {
            plain("Downwards Ratio",6); plain("Attack",5); plain("Release",75);
        }
        if (profile>=2) { plain("Upwards Ratio",3); plain("Upwards Offset",-12); plain("Downwards Offset",-6); }
        if (profile==2) { plain("Mix",0.43); }
        if (profile==3) { plain("Mix",0); }
        if (profile==4 || profile==5) { plain("Mode",profile==4?1:2); plain("SC Channel Link",0.35); }
        if (profile==7) { plain("Window Size",6); plain("Window Overlap",2); }
        if (profile==8) { plain("Window Size",15); plain("Window Overlap",5); }
        auto arrangement=channels==2?SpeakerArr::kStereo:SpeakerArr::kMono;
        SpeakerArrangement inputs[2]={arrangement,arrangement},outputs[1]={arrangement};
        check(processor->setBusArrangements(inputs,2,outputs,1),"setBusArrangements");
        for (int i=0;i<component->getBusCount(kAudio,kInput);++i) check(component->activateBus(kAudio,kInput,i,true),"activate input");
        check(component->activateBus(kAudio,kOutput,0,true),"activate output");
        ProcessSetup setup{}; setup.processMode=kOffline; setup.symbolicSampleSize=kSample32; setup.maxSamplesPerBlock=block; setup.sampleRate=rate;
        check(processor->setupProcessing(setup),"setupProcessing");
        check(component->setActive(true),"setActive"); active=true;
        check(processor->setProcessing(true),"setProcessing"); processing=true;
    }
};
static float input(int n,int c,double rate,bool sidechain) {
    const int total=131072;
    if (n<1024 || n>=total-32768) return 0;
    uint32_t h=uint32_t(n)*747796405u+uint32_t(c+1)*2891336453u;
    h=((h>>((h>>28)+4))^h)*277803737u; h=(h>>22)^h;
    double noise=(double(h)/4294967296.0-0.5);
    double t=n/rate;
    if (sidechain) return float(0.15*(1+std::sin(2*3.141592653589793*2*t))*std::sin(2*3.141592653589793*(c?330:110)*t)+0.03*noise);
    if (n==1024) return c?-.75f:.9f;
    if (n<16384) return float(0.2*std::sin(2*3.141592653589793*(c?777:440)*t));
    if (n<49152) return float(0.1*std::sin(2*3.141592653589793*(80*t+700*t*t))+0.15*noise);
    return float(0.25*noise+((n%4096)<12?0.6:0));
}
int main(int argc,char** argv) {
    try {
        if (argc!=3 && argc!=4) throw std::runtime_error("Usage: vst3_parity original-binary custom-binary [--negative-control|--mono-probe]");
        const bool negative=argc==4 && std::string(argv[3])=="--negative-control";
        const bool mono=argc==4 && std::string(argv[3])=="--mono-probe";
        if(argc==4 && !negative && !mono) throw std::runtime_error("Unknown test option");
        std::cout<<"VST3 float32 bit comparison; deterministic silence, impulse, tones, sweep, noise, transients, stereo, sidechain and tail\n";
        { Plugin a(argv[1]),b(argv[2]);
          std::cout<<"ORIGINAL factory name: "<<a.name<<"\nCUSTOM factory name: "<<b.name<<std::endl;
          if (b.params.size()<a.params.size()) throw std::runtime_error("Parameter count mismatch");
          if(b.params.size()>a.params.size()) {
            auto ag=std::find_if(b.params.begin(),b.params.end(),[](const auto& p){return text16(p.title)=="Auto Gain Compensation";});
            if(ag==b.params.end() || ag->defaultNormalizedValue!=0.0) throw std::runtime_error("New auto gain parameter must default off");
          }
          for (const auto& x:a.params) {
            auto it=std::find_if(b.params.begin(),b.params.end(),[&](const auto& p){return p.id==x.id;});
            if(it==b.params.end()) throw std::runtime_error("Missing original parameter ID");
            auto y=*it;
            if (text16(x.title)!=text16(y.title) || x.stepCount!=y.stepCount || x.defaultNormalizedValue!=y.defaultNormalizedValue) throw std::runtime_error("Parameter metadata mismatch");
            std::cout<<"PARAM "<<x.id<<" "<<text16(x.title)<<" default="<<x.defaultNormalizedValue<<"\n";
          }
        }
        uint64_t totalSamples=0,totalNonzero=0,totalMismatch=0; int cases=0;
        for (double rate:{44100.,48000.,96000.}) for (int block:{64,257,512,1024}) for (int channels:{mono?1:2}) for (int profile=0;profile<(mono?1:9);++profile) {
            Plugin a(argv[1]),b(argv[2]); a.setup(rate,block,channels,profile); b.setup(rate,block,channels,profile);
            if (a.processor->getLatencySamples()!=b.processor->getLatencySamples()) throw std::runtime_error("Latency mismatch");
            std::vector<float> data[12]; for(auto& d:data)d.resize(block);
            float* main[2]={data[0].data(),data[1].data()},*side[2]={data[2].data(),data[3].data()},*outa[2]={data[4].data(),data[5].data()},*outb[2]={data[6].data(),data[7].data()};
            float* mainB[2]={data[8].data(),data[9].data()},*sideB[2]={data[10].data(),data[11].data()};
            AudioBusBuffers ins[2],insB[2],outsA[1],outsB[1];
            ins[0].numChannels=ins[1].numChannels=outsA[0].numChannels=outsB[0].numChannels=channels;
            ins[0].channelBuffers32=main; ins[1].channelBuffers32=side; outsA[0].channelBuffers32=outa; outsB[0].channelBuffers32=outb;
            insB[0]=ins[0]; insB[1]=ins[1]; insB[0].channelBuffers32=mainB; insB[1].channelBuffers32=sideB;
            uint64_t mismatch=0,nonzero=0; double maxDifference=0;
            uint64_t outputHash=14695981039346656037ull;
            for (int offset=0,bi=0;offset<131072;offset+=block,++bi) {
                int count=std::min(block,131072-offset);
                for(int c=0;c<channels;++c) for(int i=0;i<count;++i) { main[c][i]=mainB[c][i]=input(offset+i,c,rate,false); side[c][i]=sideB[c][i]=input(offset+i,c,rate,true); outa[c][i]=outb[c][i]=NAN; }
                Changes automation;
                if (profile==6 && bi%17==0) {
                    for (int sample:{0,count/2,count-1}) {
                        automation.add(a.id("Mix"),sample,0.15+0.7*((bi+sample)%11)/10.0);
                        automation.add(a.id("Downwards Ratio"),sample,0.02+0.3*((bi+sample)%7)/6.0);
                        automation.add(a.id("Upwards Ratio"),sample,0.01+0.2*((bi+sample)%5)/4.0);
                    }
                }
                ProcessData pa; pa.processMode=kOffline; pa.symbolicSampleSize=kSample32; pa.numSamples=count; pa.numInputs=2; pa.numOutputs=1; pa.inputs=ins; pa.outputs=outsA; pa.inputParameterChanges=&automation;
                ProcessData pb=pa; pb.outputs=outsB; pb.inputs=insB;
                check(a.processor->process(pa),"original process"); check(b.processor->process(pb),"custom process");
                // Prove the checker rejects a one-bit difference, with no epsilon.
                if(negative && offset==0) { uint32_t bits; std::memcpy(&bits,&outb[0][0],4); bits^=1u; std::memcpy(&outb[0][0],&bits,4); }
                for(int c=0;c<channels;++c) for(int i=0;i<count;++i) {
                    float x=outa[c][i],y=outb[c][i]; uint32_t xb,yb; std::memcpy(&xb,&x,4); std::memcpy(&yb,&y,4);
                    if (!std::isfinite(x)||!std::isfinite(y)) throw std::runtime_error("Nonfinite/unwritten output");
                    if (xb!=yb) { ++mismatch; maxDifference=std::max(maxDifference,std::abs(double(x)-y)); }
                    outputHash=(outputHash^xb)*1099511628211ull;
                    if(x!=0)++nonzero;
                }
            }
            if (!nonzero) throw std::runtime_error("All-silent render");
            ++cases; totalSamples+=131072ull*channels; totalNonzero+=nonzero; totalMismatch+=mismatch;
            std::cout<<"CASE rate="<<rate<<" block="<<block<<" channels="<<channels<<" profile="<<profile<<" latency="<<a.processor->getLatencySamples()<<" samples="<<131072*channels<<" nonzero="<<nonzero<<" bit_mismatches="<<mismatch<<" max_abs_diff="<<std::setprecision(17)<<maxDifference<<" output_hash="<<std::hex<<outputHash<<std::dec<<std::endl;
            if(negative) { std::cout<<"NEGATIVE CONTROL: introduced one-bit difference; detected="<<mismatch<<std::endl; return mismatch==1?1:2; }
        }
        std::cout<<"TOTAL cases="<<cases<<" compared_samples="<<totalSamples<<" nonzero_samples="<<totalNonzero<<" bit_mismatches="<<totalMismatch<<" RESULT="<<(totalMismatch?"FAIL":"PASS")<<std::endl;
        return totalMismatch?1:0;
    } catch (const std::exception& e) { std::cerr<<"ERROR: "<<e.what()<<std::endl; return 2; }
}
