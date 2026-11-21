package com.example.smoke;
import java.util.List;
import java.util.ArrayList;
import static java.lang.Math.max;
import java.util.*;


public class Unformatted
{
    private int x=1;   // trailing comment
    private List<String>names=new ArrayList<>( );


    public int   compute(int a,int b){
        if(a>b){return a-b;}
        else{
            return max(a,b)+x;
        }
    }
    /* block comment */
    public void loop( ) {
        for(int i=0;i<10;i++){names.add( "n"+i );}
        // own-line comment
        int[] values=new int[]{1,2,3};
        String s=names.isEmpty()?"empty":names.get(0);
    }
}
