// Internal VST3 host: exercises the actual plugin, writes reference/output WAVs.
#pragma push_macro("main")
#undef main
#define main parity_test_main
#include "vst3_parity.cpp"
#undef main
#pragma pop_macro("main")
#include "pluginterfaces/base/ibstream.h"
#include <filesystem>
#include <fstream>
#include <regex>
struct Memory : IBStream {
    UNKNOWN_METHODS(IBStream)
    std::vector<char> bytes; size_t position=0;
    tresult PLUGIN_API read(void* out,int32 count,int32* got) override {
        size_t n=std::min(size_t(count),bytes.size()-std::min(position,bytes.size()));
        std::memcpy(out,bytes.data()+position,n);position+=n;if(got)*got=int32(n);return kResultOk;
    }
    tresult PLUGIN_API write(void* in,int32 count,int32* wrote) override {
        bytes.resize(std::max(bytes.size(),position+count));std::memcpy(bytes.data()+position,in,count);position+=count;if(wrote)*wrote=count;return kResultOk;
    }
    tresult PLUGIN_API seek(int64 pos,int32 mode,int64* result) override {
        int64 next=pos+(mode==kIBSeekCur?position:mode==kIBSeekEnd?bytes.size():0);
        if(next<0 || next>int64(bytes.size()))return kInvalidArgument;position=size_t(next);if(result)*result=next;return kResultOk;
    }
    tresult PLUGIN_API tell(int64* result) override {*result=position;return kResultOk;}
};
static Memory state(Plugin& p) {Memory m;check(p.component->getState(&m),"getState");m.position=0;return m;}
static double gainDb(Plugin& p) {
    auto id=p.id("Output Gain");
    double gain=p.controller->normalizedParamToPlain(id,p.controller->getParamNormalized(id));
    auto m=state(p);std::string saved(m.bytes.begin(),m.bytes.end());
    if(saved.find("auto-gain-db")!=std::string::npos)throw std::runtime_error("Obsolete second gain state is still present");
    return 20*std::log10(gain);
}
static float signal(int64_t n,int c,double rate,bool side=false) {
    if(n<0)return 0;double t=n/rate;
    uint32_t h=uint32_t(n)*747796405u+uint32_t(c+1)*2891336453u;h=((h>>((h>>28)+4))^h)*277803737u;h=(h>>22)^h;
    double noise=double(h)/4294967296.0-.5;
    if(side)return float(.08*std::sin(2*M_PI*(c?500:100)*t)+.01*noise);
    return float(.11*std::sin(2*M_PI*(c?880:440)*t)+.04*std::sin(2*M_PI*(c?3300:2200)*t)+.025*noise);
}
struct Packet {
    std::vector<float> samples[6];float* input[2];float* side[2];float* output[2];AudioBusBuffers ins[2],outs[1];
    explicit Packet(int block) {
        for(auto& v:samples)v.resize(block);
        input[0]=samples[0].data();input[1]=samples[1].data();side[0]=samples[2].data();side[1]=samples[3].data();output[0]=samples[4].data();output[1]=samples[5].data();
        ins[0].numChannels=ins[1].numChannels=outs[0].numChannels=2;ins[0].channelBuffers32=input;ins[1].channelBuffers32=side;outs[0].channelBuffers32=output;
    }
    void process(Plugin& p,int64_t offset,int count,double rate,Changes& changes,bool silence=false) {
        for(int c=0;c<2;++c)for(int i=0;i<count;++i) {input[c][i]=silence?0:signal(offset+i,c,rate);side[c][i]=silence?0:signal(offset+i,c,rate,true);output[c][i]=NAN;}
        ProcessData d;d.processMode=kOffline;d.symbolicSampleSize=kSample32;d.numSamples=count;d.numInputs=2;d.numOutputs=1;d.inputs=ins;d.outputs=outs;d.inputParameterChanges=&changes;
        check(p.processor->process(d),"process");
        for(int c=0;c<2;++c)for(int i=0;i<count;++i)if(!std::isfinite(output[c][i]))throw std::runtime_error("Nonfinite output");
    }
};
static void wav(const std::filesystem::path& path,const std::vector<float>& values,int rate,int channels) {
    std::ofstream f(path,std::ios::binary);auto u32=[&](uint32_t v){f.write(reinterpret_cast<const char*>(&v),4);};auto u16=[&](uint16_t v){f.write(reinterpret_cast<const char*>(&v),2);};
    f.write("RIFF",4);u32(uint32_t(values.size()*4+36));f.write("WAVEfmt ",8);u32(16);u16(3);u16(channels);u32(rate);u32(rate*channels*4);u16(channels*4);u16(32);f.write("data",4);u32(values.size()*4);f.write(reinterpret_cast<const char*>(values.data()),values.size()*4);
}
static void automate(Plugin& p,Changes& changes,const std::string& name,double value,int offset=0) {
    auto id=p.id(name);changes.add(id,offset,p.controller->plainParamToNormalized(id,value));
}
static void stop(Plugin& p) {check(p.processor->setProcessing(false),"stop processing");p.processing=false;check(p.component->setActive(false),"deactivate");p.active=false;}
int main(int argc,char** argv) {
    try {
        if(argc!=3)throw std::runtime_error("Usage: auto_gain_host custom-vst3-binary render-folder");
        std::filesystem::path folder(argv[2]);std::filesystem::create_directories(folder);
        const std::vector<std::pair<std::string,double>> steps={
            {"Global Threshold",-24},{"Global Threshold",-6},{"Output Gain",.25},{"Mix",.35},
            {"Threshold Center",3000},{"Threshold Slope",3},{"Threshold Curve",-.5},
            {"Upwards Offset",-18},{"Upwards Ratio",5},{"Upwards Hi-Freq Rolloff",.2},{"Upwards Knee",18},
            {"Downwards Offset",-8},{"Downwards Ratio",12},{"Downwards Hi-Freq Rolloff",.4},{"Downwards Knee",18},
            {"Attack",12},{"Release",40},{"Window Size",12},{"Window Overlap",5},
            {"Mode",1},{"SC Channel Link",.2},{"Mode",2}};
        std::ofstream manifest(folder/"manifest.tsv");manifest<<"stem\trate\tblock\tstep_time\tduration\tparameter\tvalue\tretained_gain_db\n";
        int cases=0;
        for(int rate:{44100,48000,96000})for(int block:{64,257,1024}) {
            Plugin p(argv[1]);p.plain("Downwards Ratio",6);p.plain("Upwards Ratio",2);p.plain("Attack",5);p.plain("Release",40);p.plain("Auto Gain Compensation",1);p.setup(rate,block,2,0);
            Packet packet(block);int64_t offset=0;
            auto render=[&](int frames,const std::string& parameter,double value,std::vector<float>* ref,std::vector<float>* out,bool silence=false,int eventOffset=0) {
                int done=0;
                while(done<frames) {
                    int count=std::min(block,frames-done);Changes changes;
                    if(done==0&&!parameter.empty())automate(p,changes,parameter,value,eventOffset);
                    packet.process(p,offset,count,rate,changes,silence);
                    auto latency=p.processor->getLatencySamples();
                    if(ref&&out)for(int i=0;i<count;++i)for(int c=0;c<2;++c) {ref->push_back(signal(offset+i-latency,c,rate));out->push_back(packet.output[c][i]);}
                    done+=count;offset+=count;
                }
            };
            render(rate*2,"",0,nullptr,nullptr);
            int scenario=0;
            // Full parameter sweep at 48k/257; other configurations test threshold and output gain.
            auto selected=(rate==48000&&block==257)?steps:std::vector<std::pair<std::string,double>>{{"Global Threshold",-24},{"Global Threshold",-6},{"Output Gain",.25}};
            for(auto [name,value]:selected) {
                std::vector<float> ref,out;
                // .5s pre-change + 2s post-change: FFmpeg measures the actual settled signal.
                render(rate/2,"",0,&ref,&out);render(rate*2,name,value,&ref,&out);
                std::string stem="r"+std::to_string(rate)+"-b"+std::to_string(block)+"-s"+std::to_string(scenario++);
                wav(folder/(stem+"-input.wav"),ref,rate,2);wav(folder/(stem+"-output.wav"),out,rate,2);
                double db=gainDb(p);manifest<<stem<<"\t"<<rate<<"\t"<<block<<"\t0.5\t2.5\t"<<name<<"\t"<<value<<"\t"<<db<<"\n";
                std::cout<<"RENDER "<<stem<<" parameter="<<name<<" gain_db="<<db<<std::endl;++cases;
            }
            double before=gainDb(p);
            // Toggle partway through a block: no continued gain movement after that sample.
            render(block,"Auto Gain Compensation",0,nullptr,nullptr,false,17);
            double frozen=gainDb(p);
            render(rate*2,"Downwards Ratio",2,nullptr,nullptr);
            render(rate,"Global Threshold",-30,nullptr,nullptr);
            double after=gainDb(p);if(frozen!=after)throw std::runtime_error("Gain moved while disabled");
            std::cout<<"FREEZE rate="<<rate<<" block="<<block<<" before="<<before<<" frozen="<<frozen<<" after_parameter_changes="<<after<<" PASS\n";
            render(rate,"Output Gain",2,nullptr,nullptr);
            double manual=gainDb(p);
            if(std::abs(manual-20*std::log10(2.0))>0.001)throw std::runtime_error("Manual Output Gain did not take over while auto was off");
            frozen=manual;
            std::cout<<"MANUAL_OUTPUT_GAIN rate="<<rate<<" block="<<block<<" db="<<manual<<" PASS\n";
            Memory saved=state(p);stop(p);
            Plugin restored(argv[1]);check(restored.component->setState(&saved),"restore state");
            if(gainDb(restored)!=frozen)throw std::runtime_error("Frozen gain not restored");
            if(restored.controller->getParamNormalized(restored.id("Auto Gain Compensation"))!=0)throw std::runtime_error("Toggle not restored off");
            restored.setup(rate,block,2,0);Packet restoredPacket(block);Changes none;
            for(int i=0;i<rate/block+1;++i)restoredPacket.process(restored,int64_t(i)*block,block,rate,none);
            if(gainDb(restored)!=frozen)throw std::runtime_error("Restored gain did not remain frozen");
            std::cout<<"STATE_RESTORE rate="<<rate<<" block="<<block<<" frozen_gain="<<frozen<<" PASS\n";
            // Silence does not introduce any new correction after the momentary window drains.
            automate(restored,none,"Auto Gain Compensation",1);restoredPacket.process(restored,0,block,rate,none,true);Changes empty;
            for(int i=0;i<rate/block*2;++i)restoredPacket.process(restored,0,block,rate,empty,true);
            double silenceGain=gainDb(restored);
            for(int i=0;i<rate/block;++i)restoredPacket.process(restored,0,block,rate,empty,true);
            if(gainDb(restored)!=silenceGain)throw std::runtime_error("Gain changed during silence");
            std::cout<<"SILENCE rate="<<rate<<" block="<<block<<" held_gain="<<silenceGain<<" PASS\n";
        }
        std::cout<<"HOST PASS rendered_cases="<<cases<<" freeze=9 state_restore=9 silence=9\n";return 0;
    }catch(const std::exception& e){std::cerr<<"ERROR: "<<e.what()<<std::endl;return 1;}
}
